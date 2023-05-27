use crate::{
    player::{
        controls::{Action, Controls},
        error::Error,
        notification::{BroadcastReceiver, BroadcastSender, Notification},
    },
    state::{
        app::{SafePlayerState, SkipDirection},
        ClockValue, StatusValue, TrackListType, TrackListValue,
    },
    REFRESH_RESOLUTION,
};
use flume::Receiver;
use futures::prelude::*;
use gst::{bus::BusStream, ClockTime, Element, MessageView, SeekFlags, State as GstState};
use gstreamer::{self as gst, prelude::*};
use hifirs_qobuz_api::client::{
    self,
    album::Album,
    api::Client,
    playlist::Playlist,
    track::{Track, TrackListTrack, TrackStatus},
    AudioQuality, UrlType,
};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::{select, sync::RwLock};

#[macro_use]
pub mod controls;
pub mod error;
pub mod notification;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A player handles playing media to a device.
#[derive(Debug, Clone)]
pub struct Player {
    /// Used to broadcast the player state out to other components.
    playbin: Element,
    /// The app state to save player inforamtion into.
    /// Qobuz client
    client: Client,
    state: SafePlayerState,
    controls: Controls,
    quit_when_done: bool,
    notify_sender: BroadcastSender,
    notify_receiver: BroadcastReceiver,
    about_to_finish_rx: Receiver<bool>,
}

type SafePlayer = Arc<RwLock<Player>>;

pub async fn new(client: Client, state: SafePlayerState, quit_when_done: bool) -> Result<Player> {
    gst::init()?;

    let playbin = gst::ElementFactory::make("playbin3").build()?;

    let (about_to_finish_tx, about_to_finish_rx) = flume::bounded::<bool>(1);
    let (mut notify_sender, notify_receiver) = async_broadcast::broadcast(3);
    notify_sender.set_overflow(true);

    // Connects to the `about-to-finish` signal so the player
    // can setup the next track to play. Enables gapless playback.
    playbin.connect("about-to-finish", false, move |_| {
        debug!("about to finish");
        about_to_finish_tx
            .send(true)
            .expect("failed to send about to finish message");

        None
    });

    let controls = Controls::new();

    let player = Player {
        client,
        playbin,
        controls,
        state,
        quit_when_done,
        notify_sender,
        notify_receiver,
        about_to_finish_rx,
    };

    Ok(player)
}

impl Player {
    /// Play the player.
    pub async fn play(&self, wait: bool) -> Result<()> {
        self.set_player_state(gst::State::Playing, wait).await?;
        Ok(())
    }
    /// Pause the player.
    pub async fn pause(&self, wait: bool) -> Result<()> {
        self.set_player_state(gst::State::Paused, wait).await?;
        Ok(())
    }
    /// Ready the player.
    pub async fn ready(&self, wait: bool) -> Result<()> {
        self.set_player_state(gst::State::Ready, wait).await?;
        Ok(())
    }
    /// Stop the player.
    pub async fn stop(&self, wait: bool) -> Result<()> {
        self.set_player_state(gst::State::Null, wait).await?;
        Ok(())
    }
    /// Sets the player to a specific state.
    pub async fn set_player_state(&self, state: gst::State, wait: bool) -> Result<()> {
        self.playbin.set_state(state)?;

        if wait {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            while self.current_state() != state.into() {
                debug!(
                    "waiting for player to change to {}",
                    self.current_state().as_str()
                );
                interval.tick().await;
            }
        }

        Ok(())
    }
    /// Toggle play and pause.
    pub async fn play_pause(&self) -> Result<()> {
        let mut state = self.state.write().await;

        if self.is_playing() {
            state.set_target_status(GstState::Paused);
            self.pause(true).await?;
        } else if self.is_paused() || self.is_ready() {
            state.set_target_status(GstState::Playing);
            self.play(true).await?;
        }

        Ok(())
    }
    /// Retreive the current player state.
    pub fn state(&self) -> SafePlayerState {
        self.state.clone()
    }
    /// Is the player paused?
    pub fn is_paused(&self) -> bool {
        self.playbin.current_state() == gst::State::Paused
    }
    /// Is the player playing?
    pub fn is_playing(&self) -> bool {
        self.playbin.current_state() == gst::State::Playing
    }
    /// Is the player ready?
    pub fn is_ready(&self) -> bool {
        self.playbin.current_state() == gst::State::Ready
    }
    /// Current player state
    pub fn current_state(&self) -> StatusValue {
        self.playbin.current_state().into()
    }
    /// Current track position.
    pub fn position(&self) -> Option<ClockValue> {
        self.playbin
            .query_position::<ClockTime>()
            .map(|position| position.into())
    }
    /// Current track duraiton.
    pub fn duration(&self) -> Option<ClockValue> {
        self.playbin
            .query_duration::<ClockTime>()
            .map(|duration| duration.into())
    }
    /// Seek to a specified time in the current track.
    pub async fn seek(&self, time: ClockValue, flags: Option<SeekFlags>) -> Result<()> {
        let flags = if let Some(flags) = flags {
            flags
        } else {
            SeekFlags::FLUSH | SeekFlags::KEY_UNIT
        };

        self.playbin.seek_simple(flags, time.inner_clocktime())?;
        self.notify_sender
            .broadcast(Notification::Position { position: time })
            .await?;

        Ok(())
    }
    /// Load the previous player state and seek to the last known position.
    pub async fn resume(&mut self, autoplay: bool) -> Result<()> {
        let mut state = self.state.write().await;

        if state.load_last_state().await {
            state.set_resume(true);

            if autoplay {
                state.set_target_status(GstState::Playing);
            } else {
                state.set_target_status(GstState::Paused);
            }

            self.notify_sender
                .broadcast(Notification::CurrentTrackList {
                    list: state.track_list(),
                })
                .await?;

            if let Some(track) = state.current_track() {
                self.notify_sender
                    .broadcast(Notification::CurrentTrack {
                        track: track.clone(),
                    })
                    .await?;

                if let Some(url) = track.track_url {
                    self.playbin.set_property("uri", url.url);

                    self.ready(true).await?;
                    self.pause(true).await?;

                    let position = state.position();

                    self.seek(
                        position.clone(),
                        Some(SeekFlags::ACCURATE | SeekFlags::FLUSH),
                    )
                    .await?;

                    return Ok(());
                } else {
                    return Err(Error::Resume);
                }
            } else {
                return Err(Error::Resume);
            }
        }

        Ok(())
    }
    /// Retreive controls for the player.
    pub fn controls(&self) -> Controls {
        self.controls.clone()
    }
    /// Jump forward in the currently playing track +10 seconds.
    pub async fn jump_forward(&self) -> Result<()> {
        if self.state.read().await.buffering() {
            return Ok(());
        }

        // TODO: Logic for faster skipping -- debounce keypresses
        // TODO: Also applies to skipping
        // When the user jumps forward, the player should
        // - pause the player, if playing
        // - record the jump, update the state position with the new value
        // - spawn a task that starts playing again in 150ms
        // - if the user presses the button again within 150ms, the player should add
        //   another jump, cancel the previous task and reset the time to now
        // - if another button press does not come, the player should seek the playhead
        //   and return to the original status
        // - where to store handle?

        if let (Some(current_position), Some(duration)) = (
            self.playbin.query_position::<ClockTime>(),
            self.playbin.query_duration::<ClockTime>(),
        ) {
            let ten_seconds = ClockTime::from_seconds(10);
            let next_position = current_position + ten_seconds;

            if next_position < duration {
                self.seek(next_position.into(), None).await?;
            } else {
                self.seek(duration.into(), None).await?;
            }
        }

        Ok(())
    }
    /// Jump forward in the currently playing track -10 seconds.
    pub async fn jump_backward(&self) -> Result<()> {
        if self.state.read().await.buffering() {
            return Ok(());
        }

        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            if current_position.seconds() < 10 {
                self.seek(ClockTime::default().into(), None).await?;
            } else {
                let ten_seconds = ClockTime::from_seconds(10);
                let seek_position = current_position - ten_seconds;

                self.seek(seek_position.into(), None).await?;
            }
        }

        Ok(())
    }
    /// Skip to the next, previous or specific track in the playlist.
    pub async fn skip(&self, direction: SkipDirection, num: Option<usize>) -> Result<()> {
        let mut state = self.state.write().await;

        // Typical previous skip functionality where if,
        // the track is greater than 1 second into playing,
        // then it goes to the beginning. If triggered again
        // within a second after playing, it will skip to the previous track.
        if direction == SkipDirection::Backward {
            if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
                let one_second = ClockTime::from_seconds(1);

                if current_position > one_second && num.is_none() {
                    debug!("current track position >1s, seeking to start of track");

                    let zero_clock: ClockValue = ClockTime::default().into();

                    self.seek(zero_clock.clone(), None).await?;
                    state.set_position(zero_clock.clone());

                    return Ok(());
                }
            }
        }

        if !self.is_ready() {
            self.ready(true).await?;
        }

        if let Some(next_track_to_play) = state.skip_track(num, direction.clone()).await {
            if let Some(track_url) = &next_track_to_play.track_url {
                debug!("skipping {direction} to next track");

                self.playbin
                    .set_property("uri", Some(track_url.url.clone()));
                self.set_player_state(state.target_status().into(), true)
                    .await?;

                self.notify_sender
                    .broadcast(Notification::CurrentTrack {
                        track: next_track_to_play,
                    })
                    .await?;
            }
        }

        Ok(())
    }
    /// Skip to a specific track in the current playlist
    /// by its index in the list.
    pub async fn skip_to(&self, index: usize) -> Result<()> {
        let state = self.state.read().await;

        if let Some(current_index) = state.current_track_index() {
            if index > current_index {
                debug!(
                    "skipping forward from track {} to track {}",
                    current_index, index
                );
                self.skip(SkipDirection::Forward, Some(index)).await?;
            } else {
                debug!(
                    "skipping backward from track {} to track {}",
                    current_index, index
                );
                self.skip(SkipDirection::Backward, Some(index)).await?;
            }
        }

        Ok(())
    }
    /// Skip to a specific track in the current playlist, by the
    /// track id.
    pub async fn skip_to_by_id(&self, track_id: usize) -> Result<()> {
        let state = self.state.read().await;

        if let Some(track_number) = state.track_index(track_id) {
            self.skip_to(track_number).await?;
        }

        Ok(())
    }
    /// Plays a single track.
    pub async fn play_track(&self, track: Track, quality: Option<AudioQuality>) -> Result<()> {
        if self.is_playing() || self.is_paused() {
            self.ready(true).await?;
        }

        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        let mut track = TrackListTrack::new(track, Some(0), Some(1), Some(quality.clone()), None);
        track.status = TrackStatus::Playing;

        let mut queue = VecDeque::new();
        queue.push_front(track.clone());

        let mut tracklist = TrackListValue::new(Some(queue));
        tracklist.set_list_type(TrackListType::Track);

        let mut state = self.state.write().await;
        state.replace_list(tracklist.clone());

        state.attach_track_url(&mut track).await;
        state.set_current_track(track.clone());
        state.set_target_status(GstState::Playing);

        if let Some(track_url) = &track.track_url {
            self.playbin
                .set_property("uri", Some(track_url.url.to_string()));

            self.play(true).await?;

            self.notify_sender
                .broadcast(Notification::CurrentTrackList { list: tracklist })
                .await?;

            self.notify_sender
                .broadcast(Notification::CurrentTrack {
                    track: track.clone(),
                })
                .await?;
        }

        Ok(())
    }
    /// Plays a full album.
    pub async fn play_album(&self, mut album: Album, quality: Option<AudioQuality>) -> Result<()> {
        if self.is_playing() || self.is_paused() {
            self.ready(true).await?;
        }

        if album.tracks.is_none() {
            album.attach_tracks(self.client.clone()).await;
        }

        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        let mut tracklist = TrackListValue::new(album.to_tracklist(quality.clone()));
        tracklist.set_album(album.clone());
        tracklist.set_list_type(TrackListType::Album);

        let mut first_track = tracklist.front().unwrap().clone();
        first_track.status = TrackStatus::Playing;

        tracklist.set_track_status(first_track.track.id as usize, TrackStatus::Playing);

        let mut state = self.state.write().await;
        state.replace_list(tracklist.clone());

        state.attach_track_url(&mut first_track).await;
        state.set_current_track(first_track.clone());
        state.set_target_status(GstState::Playing);

        if let Some(t) = &first_track.track_url {
            self.playbin.set_property("uri", Some(t.url.as_str()));
            self.play(true).await?;

            self.notify_sender
                .broadcast(Notification::CurrentTrackList {
                    list: tracklist.clone(),
                })
                .await?;

            self.notify_sender
                .broadcast(Notification::CurrentTrack {
                    track: first_track.clone(),
                })
                .await?;

            Ok(())
        } else {
            Err(Error::TrackURL)
        }
    }
    /// Play an item from Qobuz web uri
    pub async fn play_uri(&self, uri: String, quality: Option<AudioQuality>) -> Result<()> {
        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        match client::parse_url(uri.as_str()) {
            Ok(url) => match url {
                UrlType::Album { id } => match self.client.album(&id).await {
                    Ok(album) => {
                        self.play_album(album, Some(quality)).await?;
                    }
                    Err(err) => {
                        return Err(Error::FailedToPlay {
                            message: format!(
                                "Failed to play album {id}, {err}. Is the ID correct?"
                            ),
                        });
                    }
                },
                UrlType::Playlist { id } => match self.client.playlist(id).await {
                    Ok(playlist) => {
                        self.play_playlist(playlist, Some(quality)).await?;
                    }
                    Err(err) => {
                        return Err(Error::FailedToPlay {
                            message: format!(
                                "Failed to play playlsit {id}, {err}. Is the ID correct?"
                            ),
                        })
                    }
                },
                UrlType::Track { id } => match self.client.track(id).await {
                    Ok(track) => {
                        self.play_track(track, Some(quality)).await?;
                    }
                    Err(err) => {
                        return Err(Error::FailedToPlay {
                            message: format!(
                                "Failed to play track {id}, {err}. Is the ID correct?"
                            ),
                        })
                    }
                },
            },
            Err(err) => {
                return Err(Error::FailedToPlay {
                    message: format!("Failed to play item. {err}"),
                })
            }
        }

        Ok(())
    }
    /// Plays all tracks in a playlist.
    pub async fn play_playlist(
        &self,
        mut playlist: Playlist,
        quality: Option<AudioQuality>,
    ) -> Result<()> {
        if self.is_playing() || self.is_paused() {
            self.ready(true).await?;
        }

        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        let mut tracklist = TrackListValue::new(playlist.to_tracklist(quality.clone()));
        tracklist.set_playlist(playlist.clone());
        tracklist.set_list_type(TrackListType::Playlist);

        let mut first_track = tracklist.front().unwrap().clone();
        first_track.status = TrackStatus::Playing;

        tracklist.set_track_status(first_track.track.id as usize, TrackStatus::Playing);

        let mut state = self.state.write().await;
        state.replace_list(tracklist.clone());

        state.attach_track_url(&mut first_track).await;
        state.set_current_track(first_track.clone());
        state.set_target_status(GstState::Playing);

        if let Some(t) = &first_track.track_url {
            self.playbin.set_property("uri", Some(t.url.as_str()));
            self.play(true).await?;

            self.notify_sender
                .broadcast(Notification::CurrentTrackList {
                    list: tracklist.clone(),
                })
                .await?;

            self.notify_sender
                .broadcast(Notification::CurrentTrack {
                    track: first_track.clone(),
                })
                .await?;
        }

        Ok(())
    }

    async fn prep_next_track(&self) -> Result<()> {
        let mut state = self.state.write().await;

        if let Some(next_track) = state.skip_track(None, SkipDirection::Forward).await {
            debug!("received new track, adding to player");
            if let Some(next_playlist_track_url) = &next_track.track_url {
                self.playbin
                    .set_property("uri", Some(next_playlist_track_url.url.clone()));
            }
        } else {
            debug!("no more tracks left");
        }

        Ok(())
    }

    /// Inserts the most recent position into the state at a set interval.
    pub async fn clock_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(REFRESH_RESOLUTION));
        let mut quit_receiver = self.state.read().await.quitter();

        loop {
            interval.tick().await;

            if let Ok(quit) = quit_receiver.try_recv() {
                if quit {
                    debug!("quitting clock loop");
                    return;
                }
            }
            if self.current_state() != GstState::VoidPending.into()
                || self.current_state() != GstState::Null.into()
            {
                if let Some(position) = self.position() {
                    let mut state = self.state.write().await;
                    state.set_position(position.clone());

                    self.notify_sender
                        .broadcast(Notification::Position { position })
                        .await
                        .expect("failed to send notification");
                }
            }
        }
    }

    /// Get Gstreamer message stream
    pub async fn message_stream(&self) -> BusStream {
        self.playbin.bus().unwrap().stream()
    }

    pub fn notify_receiver(&self) -> BroadcastReceiver {
        self.notify_receiver.clone()
    }

    pub fn safe(self) -> SafePlayer {
        Arc::new(RwLock::new(self))
    }
}

/// Handles messages from the player and takes necessary action.
pub async fn player_loop(
    safe_player: Arc<RwLock<Player>>,
    client: Client,
    safe_state: SafePlayerState,
) -> Result<()> {
    let p = safe_player.read().await;
    let action_rx = p.controls.action_receiver();
    let mut messages = p.message_stream().await;
    let mut about_to_finish = p.about_to_finish_rx.stream();
    let mut quitter = safe_state.read().await.quitter();
    let mut actions = action_rx.stream();

    loop {
        select! {
            quit = quitter.recv() => {
                match quit {
                    Ok(quit) => {
                        if quit {
                            debug!("quitting player loop, exiting application");

                            let player = safe_player.read().await;

                            if player.is_playing() {
                                debug!("pausing player");
                                player.pause(true).await?;
                            }

                            if player.is_paused() {
                                debug!("readying player");
                                player.ready(true).await?;
                            }


                            if player.is_ready() {
                                debug!("stopping player");
                                player.stop(true).await?;
                            }

                            std::process::exit(0);
                        }
                    },
                    Err(_) => {
                        debug!("quitting player loop, with error");
                        break;
                    },
                }
            }
            Some(almost_done) = about_to_finish.next() => {
                if almost_done {
                    safe_player.read().await.prep_next_track().await?
                }
            }
            Some(action) = actions.next() => {
                let player = safe_player.read().await;

                match action {
                    Action::JumpBackward => player.jump_backward().await?,
                    Action::JumpForward => player.jump_forward().await?,
                    Action::Next => player.skip(SkipDirection::Forward,None).await?,
                    Action::Pause => player.pause(true).await?,
                    Action::Play => player.play(true).await?,
                    Action::PlayPause => player.play_pause().await?,
                    Action::Previous => player.skip(SkipDirection::Backward,None).await?,
                    Action::Stop => player.stop(true).await?,
                    Action::PlayAlbum { album_id } => {
                        if let Ok(album) = client.album(&album_id).await {
                            player.play_album(album, None).await?;
                        }
                    },
                    Action::PlayTrack { track_id } => {
                        if let Ok(track) = client.track(track_id).await {
                            player.play_track(track, None).await?;
                        }
                    },
                    Action::PlayUri { uri } => player.play_uri(uri, Some(client.quality())).await?,
                    Action::PlayPlaylist { playlist_id } => {
                        let playlist = client.playlist(playlist_id).await?;
                        player.play_playlist(playlist, Some(client.quality())).await?;
                    },
                    Action::Quit => safe_state.read().await.quit(),
                    Action::SkipTo { num, direction } => player.skip(direction, Some(num)).await?,
                    Action::SkipToById { track_id } => player.skip_to_by_id(track_id).await?
                }
            }
            Some(msg) = messages.next() => {
                match msg.view() {
                    MessageView::Eos(_) => {
                        debug!("END OF STREAM");
                        let player = safe_player.read().await;

                        if player.quit_when_done {
                            safe_state.read().await.quit();
                        } else {
                            player.pause(true).await?;

                            let mut state = safe_state.write().await;
                            state.reset_player();

                            player.skip_to(0).await?;
                        }
                    },
                    MessageView::AsyncDone(msg) => {
                        let player = safe_player.read().await;

                        let position = if let Some(p)= msg.running_time() {
                            p.into()
                        } else if let Some(p) = player.position() {
                            p
                        } else {
                            ClockTime::default().into()
                        };

                        player.notify_sender.broadcast(Notification::Position { position }).await?;
                    }
                    MessageView::StreamStart(_) => {
                        debug!("stream start");
                        let player = safe_player.read().await;

                        if let Some(current_track) = safe_state.read().await.current_track() {
                            player.notify_sender
                                .broadcast(Notification::CurrentTrack { track: current_track })
                                .await?;
                        }

                        if let Some(duration) = player.duration() {
                            debug!("setting track duration");
                            let mut state = safe_state.write().await;
                            state.set_duration(duration.clone());

                            player.notify_sender.broadcast(Notification::Duration { duration }).await?;
                        }

                        player.set_player_state(safe_state.read().await.target_status().into(), true).await?;
                    }
                    MessageView::Buffering(buffering) => {
                        let player = safe_player.read().await;
                        let percent = buffering.percent();

                        if percent < 100 && !safe_state.read().await.buffering() {
                            debug!("buffering {}%", percent);
                            player.pause(true).await?;
                            player.notify_sender.broadcast(Notification::Buffering { is_buffering: true }).await?;

                            let mut state = safe_state.write().await;
                            state.set_buffering(true);
                        } else if percent > 99 {
                            debug!("buffering {}%", percent);
                            let mut state = safe_state.write().await;
                            state.set_buffering(false);

                            player.set_player_state(state.target_status().into(), true).await?;
                            player.notify_sender.broadcast(Notification::Buffering { is_buffering: false }).await?;
                        }
                    }
                    MessageView::StateChanged(state_changed) => {
                        let current_state = state_changed
                            .current()
                            .to_value()
                            .get::<GstState>()
                            .unwrap();

                        let player = safe_player.read().await;

                        match current_state {
                            GstState::Playing => {
                                if safe_state.read().await.status() != GstState::Playing.into() {
                                    debug!("player state changed to Playing");

                                    if safe_state.read().await.target_status() == GstState::Playing.into() {
                                        let mut state = safe_state.write().await;
                                        state.set_status(gstreamer::State::Playing.into());

                                        player.notify_sender.broadcast(Notification::Status { status: GstState::Playing.into() }).await?;
                                    }
                                }
                            }
                            GstState::Paused => {
                                if safe_state.read().await.status() != GstState::Paused.into() {
                                    debug!("player state changed to Paused");

                                    if safe_state.read().await.target_status() == GstState::Paused.into() {
                                        let mut state = safe_state.write().await;
                                        state.set_status(gstreamer::State::Paused.into());

                                        player.notify_sender.broadcast(Notification::Status { status: GstState::Paused.into() }).await?;
                                    }
                                }
                            }
                            GstState::Ready => {
                                if safe_state.read().await.status() != GstState::Ready.into() {
                                    debug!("player state changed to Ready");

                                    if safe_state.read().await.target_status() == GstState::Ready.into() {
                                        let mut state = safe_state.write().await;
                                        state.set_status(gstreamer::State::Ready.into());

                                        player.notify_sender.broadcast(Notification::Status { status: GstState::Ready.into() }).await?;
                                    }
                                }
                            }
                            GstState::VoidPending => {
                                if safe_state.read().await.status() != GstState::VoidPending.into() {
                                    debug!("player state changed to VoidPending");

                                    if safe_state.read().await.target_status() == GstState::VoidPending.into() {
                                        let mut state = safe_state.write().await;
                                        state.set_status(gstreamer::State::VoidPending.into());

                                        player.notify_sender.broadcast(Notification::Status { status: GstState::VoidPending.into() }).await?;
                                    }
                                }
                            },
                            GstState::Null => {
                                if safe_state.read().await.status() != GstState::Null.into() {
                                    debug!("player state changed to Null");

                                    if safe_state.read().await.target_status() == GstState::Null.into() {
                                        let mut state = safe_state.write().await;
                                        state.set_status(gstreamer::State::Null.into());

                                        player.notify_sender.broadcast(Notification::Status { status: GstState::Null.into() }).await?;
                                    }
                                }
                            },
                        }
                    }
                    MessageView::Error(err) => {
                        let player = safe_player.read().await;
                        player.notify_sender.broadcast(Notification::Error { error: err.into() }).await?;

                        debug!(
                            "Error from {:?}: {} ({:?})",
                            err.src().map(|s| s.path_string()),
                            err.error(),
                            err.debug()
                        );
                    }
                    _ => (),
                }

            }
        }
    }

    Ok(())
}

#[macro_export]
macro_rules! action {
    ($self:ident, $action:expr) => {
        if let Err(_) = $self.action_tx.send_async($action).await {
            error!("error sending action");
        }
    };
}

#[macro_export]
macro_rules! action_blocking {
    ($self:ident, $action:expr) => {
        if let Err(_) = $self.action_tx.send($action) {
            error!("error sending action");
        }
    };
}
