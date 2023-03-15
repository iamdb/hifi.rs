use crate::{
    action,
    mpris::{self, MprisPlayer, MprisTrackList},
    sql::db::Database,
    state::{
        app::{PlayerState, SafePlayerState, SkipDirection},
        ClockValue, StatusValue, TrackListType, TrackListValue,
    },
    REFRESH_RESOLUTION,
};
use flume::{Receiver, Sender};
use futures::prelude::*;
use gst::{
    bus::BusStream, glib, ClockTime, Element, MessageView, SeekFlags, State as GstState,
    StateChangeError,
};
use gstreamer::{self as gst, prelude::*};
use hifirs_qobuz_api::client::{
    self,
    album::Album,
    api::Client,
    playlist::Playlist,
    track::{Track, TrackListTrack, TrackStatus},
    AudioQuality,
};
use snafu::prelude::*;
use std::{collections::VecDeque, process, sync::Arc, time::Duration};
use tokio::{select, sync::RwLock};
use zbus::Connection;

#[derive(Snafu, Debug)]
pub enum Error {
    #[snafu(display("Failed to retrieve a track url."))]
    TrackURL,
    #[snafu(display("Failed to seek."))]
    Seek,
    #[snafu(display("Sorry, could not resume previous session."))]
    Resume,
    #[snafu(display("Gstreamer failed."))]
    GStreamer {
        message: String,
    },
    Client {
        message: String,
    },
    App,
}

impl From<glib::Error> for Error {
    fn from(value: glib::Error) -> Self {
        Error::GStreamer {
            message: value.to_string(),
        }
    }
}

impl From<glib::BoolError> for Error {
    fn from(value: glib::BoolError) -> Self {
        Error::GStreamer {
            message: value.to_string(),
        }
    }
}

impl From<StateChangeError> for Error {
    fn from(value: StateChangeError) -> Self {
        Error::GStreamer {
            message: value.to_string(),
        }
    }
}

impl From<hifirs_qobuz_api::Error> for Error {
    fn from(value: hifirs_qobuz_api::Error) -> Self {
        Error::Client {
            message: value.to_string(),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum Action {
    Play,
    Pause,
    PlayPause,
    Next,
    Previous,
    Stop,
    Quit,
    SkipTo {
        num: usize,
        direction: SkipDirection,
    },
    SkipToById {
        track_id: usize,
    },
    JumpForward,
    JumpBackward,
    PlayAlbum {
        album_id: String,
    },
    PlayTrack {
        track_id: i32,
    },
    PlayUri {
        uri: String,
    },
    PlayPlaylist {
        playlist_id: i64,
    },
}

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
    connection: Connection,
    quit_when_done: bool,
}

pub async fn new(client: Client, db: Database, quit_when_done: bool) -> Result<Player> {
    gst::init()?;

    let playbin = gst::ElementFactory::make("playbin3").build()?;

    let (about_to_finish_tx, about_to_finish_rx) = flume::bounded::<bool>(1);

    // Connects to the `about-to-finish` signal so the player
    // can setup the next track to play. Enables gapless playback.
    playbin.connect("about-to-finish", false, move |_| {
        debug!("about to finish");
        about_to_finish_tx
            .send(true)
            .expect("failed to send about to finish message");

        None
    });

    let state = Arc::new(RwLock::new(PlayerState::new(client.clone(), db)));
    let controls = Controls::new();
    let connection = mpris::init(state.clone(), controls.clone()).await;

    let player = Player {
        connection,
        client,
        playbin,
        controls,
        state,
        quit_when_done,
    };

    let p = player.clone();
    tokio::spawn(async move {
        p.clock_loop().await;
    });

    let mut p = player.clone();
    tokio::spawn(async move {
        p.player_loop(about_to_finish_rx)
            .await
            .expect("failed to start player loop");
    });

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

            if let Some(track) = state.current_track() {
                if let Some(url) = track.track_url {
                    self.playbin.set_property("uri", url.url);

                    self.ready(true).await?;
                    self.pause(true).await?;

                    let position = state.position();

                    self.seek(position, Some(SeekFlags::ACCURATE | SeekFlags::FLUSH))
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
        // Typical previous skip functionality where if,
        // the track is greater than 1 second into playing,
        // then it goes to the beginning. If triggered again
        // within a second after playing, it will skip to the previous track.
        if direction == SkipDirection::Backward {
            if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
                let one_second = ClockTime::from_seconds(1);

                if current_position > one_second && num.is_none() {
                    debug!("current track position >1s, seeking to start of track");
                    self.seek(ClockTime::default().into(), None).await?;

                    return Ok(());
                }
            }
        }

        if !self.is_ready() {
            self.ready(true).await?;
        }

        let mut state = self.state.write().await;

        if let Some(next_track_to_play) = state.skip_track(num, direction.clone()).await {
            // Need to drop state before any dbus calls.
            drop(state);

            if let Some(track_url) = next_track_to_play.track_url {
                debug!("skipping {direction} to next track");

                self.playbin.set_property("uri", Some(track_url.url));

                self.set_player_state(self.state.read().await.target_status().into(), true)
                    .await?;
                self.dbus_metadata_changed().await;
            }
        }
        Ok(())
    }
    /// Skip to a specific track in the current playlist
    /// by its index in the list.
    pub async fn skip_to(&self, index: usize) -> Result<()> {
        let state = self.state.read().await;

        if let Some(current_index) = state.current_track_index() {
            drop(state);
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
        if self.is_playing() {
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
        state.replace_list(tracklist);

        state.attach_track_url(&mut track).await;
        state.set_current_track(track.clone());
        state.set_target_status(GstState::Playing);

        if let Some(track_url) = track.track_url {
            self.playbin
                .set_property("uri", Some(track_url.url.to_string()));

            self.play(true).await?;
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
        state.replace_list(tracklist);

        state.attach_track_url(&mut first_track).await;
        state.set_current_track(first_track.clone());
        state.set_target_status(GstState::Playing);

        // Need to drop state before any dbus calls.
        drop(state);

        if let Some(t) = first_track.track_url {
            self.playbin.set_property("uri", Some(t.url.as_str()));
            self.play(true).await?;

            if let Some(tracks) = album.tracks {
                let tracks = tracks
                    .items
                    .iter()
                    .map(|t| t.id.to_string())
                    .collect::<Vec<String>>();

                let current = tracks.first().cloned().unwrap();

                self.dbus_track_list_replaced_signal(tracks, current)
                    .await?;
            }

            self.dbus_metadata_changed().await;
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
                client::UrlType::Album { id } => match self.client.album(&id).await {
                    Ok(album) => {
                        self.play_album(album, Some(quality)).await?;
                    }
                    Err(err) => {
                        println!("Failed to play album {id}, {err}. Is the ID correct?");
                        process::exit(1);
                    }
                },
                client::UrlType::Playlist { id } => match self.client.playlist(id).await {
                    Ok(playlist) => {
                        self.play_playlist(playlist, Some(quality)).await?;
                    }
                    Err(err) => {
                        println!("Failed to play playlsit {id}, {err}. Is the ID correct?");
                        process::exit(1);
                    }
                },
                client::UrlType::Track { id } => match self.client.track(id).await {
                    Ok(track) => {
                        self.play_track(track, Some(quality)).await?;
                    }
                    Err(err) => {
                        println!("Failed to play track {id}, {err}. Is the ID correct?");
                        process::exit(1);
                    }
                },
            },
            Err(err) => {
                println!("Failed to play item, {err}.");
                process::exit(1);
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
        state.replace_list(tracklist);

        state.attach_track_url(&mut first_track).await;
        state.set_current_track(first_track.clone());
        state.set_target_status(GstState::Playing);

        // Need to drop state before any dbus calls.
        drop(state);

        self.playbin
            .set_property("uri", Some(first_track.track_url.unwrap().url.as_str()));
        self.play(true).await?;

        if let Some(tracks) = playlist.tracks {
            let tracks = tracks
                .items
                .iter()
                .map(|t| t.id.to_string())
                .collect::<Vec<String>>();

            let current = tracks.first().cloned().unwrap();

            self.dbus_track_list_replaced_signal(tracks, current)
                .await?;
        }

        self.dbus_metadata_changed().await;

        Ok(())
    }
    /// Handles messages from the player and takes necessary action.
    async fn player_loop(&mut self, about_to_finish_rx: Receiver<bool>) -> Result<()> {
        let action_rx = self.controls.action_receiver();
        let mut messages = self.message_stream().await;
        let mut quitter = self.state.read().await.quitter();
        let mut actions = action_rx.stream();
        let mut about_to_finish = about_to_finish_rx.stream();

        loop {
            select! {
                quit = quitter.recv() => {
                    match quit {
                        Ok(quit) => {
                            if quit {
                                debug!("quitting player loop, exiting application");

                                if self.is_playing() {
                                    debug!("pausing player");
                                    self.pause(true).await?;
                                }

                                if self.is_paused() {
                                    debug!("readying player");
                                    self.ready(true).await?;
                                }


                                if self.is_ready() {
                                    debug!("stopping player");
                                    self.stop(true).await?;
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
                        self.prep_next_track().await?
                    }
                }
                Some(action) = actions.next() => {
                    match action {
                        Action::JumpBackward => self.jump_backward().await?,
                        Action::JumpForward => self.jump_forward().await?,
                        Action::Next => self.skip(SkipDirection::Forward,None).await?,
                        Action::Pause => self.pause(true).await?,
                        Action::Play => self.play(true).await?,
                        Action::PlayPause => self.play_pause().await?,
                        Action::Previous => self.skip(SkipDirection::Backward,None).await?,
                        Action::Stop => self.stop(true).await?,
                        Action::PlayAlbum { album_id } => {
                            if let Ok(album) = self.client.album(&album_id).await {
                                self.play_album(album, None).await?;
                            }
                        },
                        Action::PlayTrack { track_id } => {
                            if let Ok(track) = self.client.track(track_id).await {
                                self.play_track(track, None).await?;
                            }
                        },
                        Action::PlayUri { uri } => self.play_uri(uri, Some(self.client.quality())).await?,
                        Action::PlayPlaylist { playlist_id } => {
                            let playlist = self.client.playlist(playlist_id).await?;
                            self.play_playlist(playlist, Some(self.client.quality())).await?;
                        },
                        Action::Quit => self.state.read().await.quit(),
                        Action::SkipTo { num, direction } => self.skip(direction, Some(num)).await?,
                        Action::SkipToById { track_id } => self.skip_to_by_id(track_id).await?
                    }
                }
                Some(msg) = messages.next() => {
                    match msg.view() {
                        MessageView::Eos(_) => {
                            debug!("END OF STREAM");

                            if self.quit_when_done {
                                self.state.read().await.quit();
                            } else {
                                self.pause(true).await?;
                                self.state().write().await.reset_player();
                                self.skip_to(0).await?;
                            }
                        },
                        MessageView::AsyncDone(msg) => {
                            let position = if let Some(p)= msg.running_time() {
                                p.into()
                            } else if let Some(p) = self.position() {
                                p
                            } else {
                                ClockTime::default().into()
                            };

                            self.dbus_seeked_signal(Some(position)).await;
                        }
                        MessageView::StreamStart(_) => {
                            debug!("stream start");
                            if let Some(duration) = self.duration() {
                                debug!("setting track duration");
                                let mut state = self.state.write().await;
                                state.set_duration(duration);

                                drop(state);

                                self.dbus_metadata_changed().await;
                            }

                            self.set_player_state(self.state.read().await.target_status().into(), true).await?;
                        }
                        MessageView::Buffering(buffering) => {
                            let percent = buffering.percent();

                            debug!("buffering {}%", percent);

                            if !self.state.read().await.buffering() && percent < 100 {
                                if !self.is_paused() {
                                    self.pause(true).await?;
                                }

                                let mut state = self.state.write().await;
                                state.set_buffering(true);
                                drop(state);
                            } else if self.state.read().await.buffering() && percent > 99 {
                                let mut state = self.state.write().await;
                                state.set_buffering(false);

                                self.set_player_state(state.target_status().into(), true).await?;

                                drop(state);
                            }
                        }
                        MessageView::StateChanged(state_changed) => {
                            let current_state = state_changed
                                .current()
                                .to_value()
                                .get::<GstState>()
                                .unwrap();

                            let iface_ref = self.player_iface().await;
                            let iface = iface_ref.get_mut().await;

                            match current_state {
                                GstState::Playing => {
                                    if self.state.read().await.status() != GstState::Playing.into() {
                                        debug!("player state changed to Playing");

                                        if self.state.read().await.target_status() == GstState::Playing.into() {
                                            let mut state = self.state.write().await;
                                            state.set_status(gstreamer::State::Playing.into());

                                            // Need to drop state before any dbus calls.
                                            drop(state);

                                            iface
                                                .playback_status_changed(iface_ref.signal_context())
                                                .await
                                                .expect("failed");
                                        }
                                    }
                                }
                                GstState::Paused => {
                                    if self.state.read().await.status() != GstState::Paused.into() {
                                        debug!("player state changed to Paused");

                                        if self.state.read().await.target_status() == GstState::Paused.into() {
                                            let mut state = self.state.write().await;
                                            state.set_status(gstreamer::State::Paused.into());

                                            // Need to drop state before any dbus calls.
                                            drop(state);

                                            iface
                                                .playback_status_changed(iface_ref.signal_context())
                                                .await
                                                .expect("failed");
                                        }
                                    }
                                }
                                GstState::Ready => {
                                    if self.state.read().await.status() != GstState::Ready.into() {
                                        debug!("player state changed to Ready");

                                        if self.state.read().await.target_status() == GstState::Ready.into() {
                                            let mut state = self.state.write().await;
                                            state.set_status(gstreamer::State::Ready.into());

                                            // Need to drop state before any dbus calls.
                                            drop(state);

                                            iface
                                                .playback_status_changed(iface_ref.signal_context())
                                                .await
                                                .expect("failed");
                                        }
                                    }
                                }
                                GstState::VoidPending => {
                                    if self.state.read().await.status() != GstState::VoidPending.into() {
                                        debug!("player state changed to VoidPending");

                                        if self.state.read().await.target_status() == GstState::VoidPending.into() {
                                            let mut state = self.state.write().await;
                                            state.set_status(gstreamer::State::VoidPending.into());

                                            // Need to drop state before any dbus calls.
                                            drop(state);

                                            iface
                                                .playback_status_changed(iface_ref.signal_context())
                                                .await
                                                .expect("failed");
                                        }
                                    }
                                },
                                GstState::Null => {
                                    if self.state.read().await.status() != GstState::Null.into() {
                                        debug!("player state changed to Null");

                                        if self.state.read().await.target_status() == GstState::Null.into() {
                                            let mut state = self.state.write().await;
                                            state.set_status(gstreamer::State::Null.into());

                                            // Need to drop state before any dbus calls.
                                            drop(state);

                                            iface
                                                .playback_status_changed(iface_ref.signal_context())
                                                .await
                                                .expect("failed");
                                        }
                                    }
                                },
                            }
                        }
                        MessageView::Error(err) => {
                            println!(
                                "Error from {:?}: {} ({:?})",
                                err.src().map(|s| s.path_string()),
                                err.error(),
                                err.debug()
                            );
                            break;
                        }
                        _ => (),
                    }

                }
            }
        }

        Ok(())
    }
    /// Inserts the most recent position, duration and progress values into the state
    /// at a set interval.
    async fn clock_loop(&self) {
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
                    let position = position.inner_clocktime();

                    let mut state = self.state.write().await;
                    state.set_position(position.into());

                    drop(state);
                }
            }
        }
    }
    async fn dbus_track_list_replaced_signal(
        &self,
        tracks: Vec<String>,
        current: String,
    ) -> Result<()> {
        debug!("replacing dbus tracklist");
        let object_server = self.connection.object_server();

        let iface_ref = object_server
            .interface::<_, MprisTrackList>("/org/mpris/MediaPlayer2")
            .await
            .expect("failed to get object server");

        MprisTrackList::track_list_replaced(iface_ref.signal_context(), tracks, current)
            .await
            .expect("failed to send track list replaced signal");

        Ok(())
    }
    async fn player_iface(&self) -> zbus::InterfaceRef<MprisPlayer> {
        let object_server = self.connection.object_server();

        object_server
            .interface::<_, MprisPlayer>("/org/mpris/MediaPlayer2")
            .await
            .expect("failed to get object server")
    }
    async fn dbus_seeked_signal(&self, position: Option<ClockValue>) {
        let position = if let Some(p) = position {
            p
        } else if let Some(p) = self.position() {
            p
        } else {
            ClockTime::default().into()
        };

        let iface_ref = self.player_iface().await;

        mpris::MprisPlayer::seeked(
            iface_ref.signal_context(),
            position.inner_clocktime().useconds() as i64,
        )
        .await
        .expect("failed to send seeked signal");
    }

    async fn dbus_metadata_changed(&self) {
        debug!("dbus metadata changed");
        let iface_ref = self.player_iface().await;
        let iface = iface_ref.get_mut().await;

        iface
            .metadata_changed(iface_ref.signal_context())
            .await
            .expect("failed to signal metadata change");
    }

    /// Sets up basic functionality for the player.
    async fn prep_next_track(&self) -> Result<()> {
        let mut state = self.state.write().await;

        if let Some(next_track) = state.skip_track(None, SkipDirection::Forward).await {
            debug!("received new track, adding to player");
            if let Some(next_playlist_track_url) = next_track.track_url {
                self.playbin
                    .set_property("uri", Some(next_playlist_track_url.url));
            }
        } else {
            debug!("no more tracks left");
        }

        Ok(())
    }

    /// Get Gstreamer message stream
    pub async fn message_stream(&self) -> BusStream {
        self.playbin.bus().unwrap().stream()
    }
}

/// Provides controls for other modules to send commands
/// to the player
#[derive(Debug, Clone)]
pub struct Controls {
    action_tx: Sender<Action>,
    action_rx: Receiver<Action>,
}

impl Controls {
    fn new() -> Controls {
        let (action_tx, action_rx) = flume::bounded::<Action>(10);

        Controls {
            action_rx,
            action_tx,
        }
    }
    pub fn action_receiver(&self) -> Receiver<Action> {
        self.action_rx.clone()
    }
    pub async fn play(&self) {
        action!(self, Action::Play);
    }
    pub async fn pause(&self) {
        action!(self, Action::Pause);
    }
    pub async fn play_pause(&self) {
        action!(self, Action::PlayPause);
    }
    pub async fn stop(&self) {
        action!(self, Action::Stop);
    }
    pub async fn quit(&self) {
        action!(self, Action::Quit)
    }
    pub async fn next(&self) {
        action!(self, Action::Next);
    }
    pub async fn previous(&self) {
        action!(self, Action::Previous);
    }
    pub async fn skip_to(&self, num: usize, direction: SkipDirection) {
        action!(self, Action::SkipTo { num, direction });
    }
    pub async fn skip_to_by_id(&self, track_id: usize) {
        action!(self, Action::SkipToById { track_id })
    }
    pub async fn jump_forward(&self) {
        action!(self, Action::JumpForward);
    }
    pub async fn jump_backward(&self) {
        action!(self, Action::JumpBackward);
    }
    pub async fn play_album(&self, album_id: String) {
        action!(self, Action::PlayAlbum { album_id });
    }
    pub async fn play_uri(&self, uri: String) {
        action!(self, Action::PlayUri { uri });
    }
    pub async fn play_track(&self, track_id: i32) {
        action!(self, Action::PlayTrack { track_id });
    }
    pub async fn play_playlist(&self, playlist_id: i64) {
        action!(self, Action::PlayPlaylist { playlist_id })
    }
}

#[macro_export]
macro_rules! action {
    ($self:ident, $action:expr) => {
        if let Err(_) = $self.action_tx.send_async($action).await {
            error!("error sending action");
        }
    };
}
