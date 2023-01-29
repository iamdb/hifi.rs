use crate::{
    action,
    mpris::{self, MprisPlayer, MprisTrackList},
    state::{
        app::{PlayerState, SafePlayerState, SkipDirection},
        ClockValue, StatusValue, TrackListType, TrackListValue,
    },
    REFRESH_RESOLUTION,
};
use flume::{Receiver, Sender};
use futures::prelude::*;
use gst::{bus::BusStream, glib, ClockTime, Element, MessageView, SeekFlags, State as GstState};
use gstreamer::{self as gst, prelude::*};
use qobuz_client::client::{
    self,
    album::Album,
    api::Client,
    playlist::Playlist,
    track::{Track, TrackListTrack, TrackStatus},
    AudioQuality, TrackURL,
};
use snafu::prelude::*;
use std::{sync::Arc, time::Duration};
use tokio::{select, sync::Mutex};
use zbus::Connection;

#[derive(Snafu, Debug)]
pub enum Error {
    #[snafu(display("Failed to retrieve a track url."))]
    TrackURL,
    #[snafu(display("Failed to seek."))]
    Seek,
    #[snafu(display("Sorry, could not resume previous session."))]
    Session,
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
    is_buffering: bool,
    _resume: bool,
}

pub async fn new(client: Client, _resume: bool) -> Player {
    gst::init().expect("Couldn't initialize Gstreamer");
    let state = Arc::new(Mutex::new(PlayerState::new(client.clone())));
    let playbin = gst::ElementFactory::make("playbin")
        .build()
        .expect("failed to create gst element");
    let controls = Controls::new();

    let connection = mpris::init(state.clone(), controls.clone()).await;

    let (about_to_finish_tx, about_to_finish_rx) = flume::bounded::<bool>(1);
    let (next_track_tx, next_track_rx) = flume::bounded::<String>(1);

    // Connects to the `about-to-finish` signal so the player
    // can setup the next track to play. Enables gapless playback.
    playbin.connect("about-to-finish", false, move |values| {
        debug!("about to finish");
        about_to_finish_tx
            .send(true)
            .expect("failed to send about to finish message");

        if let Ok(next_track_url) = next_track_rx.recv_timeout(Duration::from_secs(5)) {
            let playbin = values[0]
                .get::<glib::Object>()
                .expect("playbin \"about-to-finish\" signal values[0]");

            playbin.set_property("uri", Some(next_track_url));
        }

        None
    });

    let player = Player {
        connection,
        client,
        playbin,
        controls,
        is_buffering: false,
        _resume,
        state,
    };

    // if resume {
    //     player.resume().await.expect("failed to resume");
    // }

    let p = player.clone();
    tokio::spawn(async move {
        p.clock_loop().await;
    });

    let mut p = player.clone();
    tokio::spawn(async move {
        p.player_loop(about_to_finish_rx, next_track_tx).await;
    });

    player
}

#[allow(unused)]
impl Player {
    /// Play the player.
    pub fn play(&self) {
        self.playbin
            .set_state(gst::State::Playing)
            .expect("Unable to set the pipeline to the `Playing` state");
    }
    /// Pause the player.
    pub fn pause(&self) {
        self.playbin
            .set_state(gst::State::Paused)
            .expect("Unable to set the pipeline to the `Paused` state");
    }
    /// Toggle play and pause.
    pub fn play_pause(&self) {
        if self.is_playing() {
            self.pause();
        } else if self.is_paused() {
            self.play()
        }
    }
    /// Ready the player.
    pub fn ready(&self) {
        self.playbin
            .set_state(gst::State::Ready)
            .expect("Unable to set the pipeline to the `Ready` state");
    }
    pub fn state(&self) -> SafePlayerState {
        self.state.clone()
    }
    /// Stop the player.
    pub fn stop(&self) {
        self.playbin
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");
    }
    /// Set the uri of the track to play.
    pub fn set_uri(&self, track_url: TrackURL) {
        self.playbin
            .set_property("uri", Some(track_url.url.as_str()));
    }
    /// Is the player paused?
    pub fn is_paused(&self) -> bool {
        self.playbin.current_state() == gst::State::Paused
    }
    /// Is the player playing?
    pub fn is_playing(&self) -> bool {
        self.playbin.current_state() == gst::State::Playing
    }
    /// Current player state
    pub fn current_state(&self) -> StatusValue {
        self.playbin.current_state().into()
    }
    pub fn position(&self) -> Option<ClockValue> {
        self.playbin
            .query_position::<ClockTime>()
            .map(|position| position.into())
    }
    pub fn duration(&self) -> Option<ClockValue> {
        self.playbin
            .query_duration::<ClockTime>()
            .map(|duration| duration.into())
    }
    /// Seek to a specified time in the current track.
    pub async fn seek(&self, time: ClockValue, flags: Option<SeekFlags>) {
        let flags = if let Some(flags) = flags {
            flags
        } else {
            SeekFlags::FLUSH | SeekFlags::KEY_UNIT
        };

        match self.playbin.seek_simple(flags, time.inner_clocktime()) {
            Ok(_) => {
                self.dbus_seeked_signal(time).await;
            }
            Err(error) => {
                error!("{}", error.message);
            }
        }
    }
    // pub async fn resume(&mut self) -> Result<()> {
    //     if let (Some(playlist), Some(next_up)) = (
    //         self.db
    //             .get::<String, TrackListValue>(StateKey::Player(PlayerKey::Playlist))
    //             .await,
    //         self.db
    //             .get::<String, TrackListTrack>(StateKey::Player(PlayerKey::NextUp))
    //             .await,
    //     ) {
    //         let next_track = self.attach_track_url(next_up).await?;

    //         if let Some(track_url) = next_track.track_url {
    //             self.set_playlist(playlist);
    //             self.set_uri(track_url);

    //             if let Some(prev_playlist) = self
    //                 .db
    //                 .get::<String, TrackListValue>(StateKey::Player(PlayerKey::PreviousPlaylist))
    //                 .await
    //             {
    //                 self.set_prev_playlist(prev_playlist);

    //                 self.pause();
    //             }
    //         }
    //         Ok(())
    //     } else {
    //         debug!("nothing to resume");
    //         self.resume = false;
    //         Ok(())
    //     }
    // }
    /// Retreive controls for the player.
    pub fn controls(&self) -> Controls {
        self.controls.clone()
    }
    pub async fn reset_state(&self) {
        self.state.lock().await.reset();
    }
    /// Jump forward in the currently playing track +10 seconds.
    pub async fn jump_forward(&self) {
        if let (Some(current_position), Some(duration)) = (
            self.playbin.query_position::<ClockTime>(),
            self.playbin.query_duration::<ClockTime>(),
        ) {
            let ten_seconds = ClockTime::from_seconds(10);
            let next_position = current_position + ten_seconds;

            if next_position < duration {
                self.seek(next_position.into(), None).await
            } else {
                self.seek(duration.into(), None).await
            }
        }
    }
    /// Jump forward in the currently playing track -10 seconds.
    pub async fn jump_backward(&self) {
        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            if current_position.seconds() < 10 {
                self.seek(ClockTime::default().into(), None).await;
            } else {
                let ten_seconds = ClockTime::from_seconds(10);
                let seek_position = current_position - ten_seconds;

                self.seek(seek_position.into(), None).await;
            }
        }
    }
    /// Skip to the next, previous or specific track in the playlist.
    pub async fn skip(&self, direction: SkipDirection, num: Option<usize>) -> Result<()> {
        // If the track is greater than 1 second into playing,
        // then we just want to go back to the beginning.
        // If triggered again within a second after playing,
        // it will skip to the previous track.
        if direction == SkipDirection::Backward {
            if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
                let one_second = ClockTime::from_seconds(1);

                if current_position > one_second && num.is_none() {
                    debug!("current track position >1s, seeking to start of track");
                    self.seek(ClockTime::default().into(), None).await;

                    self.dbus_seeked_signal(ClockValue::default()).await;
                    self.dbus_metadata_changed().await;

                    return Ok(());
                }
            }
        }

        self.ready();

        let mut state = self.state.lock().await;
        state.reset_player();

        if let Some(mut next_track_to_play) = state.skip_track(num, direction.clone()).await {
            if let Some(track_url) = next_track_to_play.track_url {
                debug!("skipping {direction} to next track");

                drop(state);
                self.dbus_seeked_signal(ClockValue::default()).await;
                self.dbus_metadata_changed().await;

                self.playbin.set_property("uri", Some(track_url.url));

                self.play();
            }
        }
        Ok(())
    }
    /// Skip to a specific track in the current playlist
    /// by its index in the list.
    pub async fn skip_to(&self, index: usize) -> Result<()> {
        let state = self.state.lock().await;

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
        let state = self.state.lock().await;

        if let Some(track_number) = state.track_index(track_id) {
            self.skip_to(track_number).await?;
        }

        Ok(())
    }
    /// Plays a single track.
    pub async fn play_track(&self, track: Track, quality: Option<AudioQuality>) {
        if self.is_playing() {
            self.stop();
        }

        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        let playlist_track =
            TrackListTrack::new(track, Some(0), Some(1), Some(quality.clone()), None);

        self.start(playlist_track, quality).await;
    }
    /// Plays a full album.
    pub async fn play_album(&self, mut album: Album, quality: Option<AudioQuality>) {
        if self.is_playing() || self.is_paused() {
            self.stop();
        }

        self.reset_state().await;

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

        let mut state = self.state.lock().await;
        state.replace_list(tracklist);

        state.attach_track_url(&mut first_track).await;
        state.set_current_track(first_track.clone());

        drop(state);

        self.playbin
            .set_property("uri", Some(first_track.track_url.unwrap().url.as_str()));
        self.play();

        if let Some(tracks) = album.tracks {
            let tracks = tracks
                .items
                .iter()
                .map(|t| t.id.to_string())
                .collect::<Vec<String>>();

            let current = tracks.first().cloned().unwrap();

            self.dbus_track_list_replaced_signal(tracks, current).await;
        }

        self.dbus_metadata_changed().await;
    }
    /// Play an item from Qobuz web uri
    pub async fn play_uri(&self, uri: String, quality: Option<AudioQuality>) {
        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        if let Some(url) = client::parse_url(uri.as_str()) {
            match url {
                client::UrlType::Album { id } => {
                    if let Ok(album) = self.client.album(id).await {
                        self.play_album(album, Some(quality)).await;
                    }
                }
                client::UrlType::Playlist { id } => {
                    if let Ok(playlist) = self.client.playlist(id).await {
                        self.play_playlist(playlist, Some(quality)).await;
                    }
                }
            }
        }
    }
    pub async fn play_playlist(&self, mut playlist: Playlist, quality: Option<AudioQuality>) {
        if self.is_playing() || self.is_paused() {
            self.stop();
        }

        self.reset_state().await;

        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        if let Some(tracklist) = playlist.to_tracklist(Some(quality.clone())) {
            debug!("creating playlist");

            let mut tracklist = TrackListValue::new(Some(tracklist));
            tracklist.set_playlist(playlist.clone());

            let first_track = tracklist.front().unwrap().clone();

            let mut state = self.state.lock().await;
            state.replace_list(tracklist);
            state.set_current_track(first_track.clone());
            drop(state);

            if let Some(tracks) = playlist.tracks {
                let tracks = tracks
                    .items
                    .iter()
                    .map(|t| t.id.to_string())
                    .collect::<Vec<String>>();

                let current = tracks.first().cloned().unwrap();

                self.dbus_track_list_replaced_signal(tracks, current).await;
            }

            self.dbus_metadata_changed().await;
            self.start(first_track, quality).await;
        }
    }
    /// Starts the player.
    async fn start(&self, mut track: TrackListTrack, quality: AudioQuality) {
        debug!("starting player");
        let playbin = &self.playbin;

        if let Ok(track_url) = self
            .client
            .track_url(track.track.id, Some(quality.clone()), None)
            .await
        {
            playbin.set_property("uri", Some(track_url.url.as_str()));
            track.set_track_url(track_url);

            self.play();
        }
    }
    /// Handles messages from the player and takes necessary action.
    async fn player_loop(
        &mut self,
        about_to_finish_rx: Receiver<bool>,
        next_track_tx: Sender<String>,
    ) {
        let action_rx = self.controls.action_receiver();
        let mut messages = self.message_stream().await;
        let mut quitter = self.state.lock().await.quitter();
        let mut actions = action_rx.stream();
        let mut about_to_finish = about_to_finish_rx.stream();

        loop {
            select! {
                quit = quitter.recv() => {
                    match quit {
                        Ok(quit) => {
                            if quit {
                                debug!("quitting");
                                break;
                            }
                        },
                        Err(_) => {
                            debug!("quitting, with error");
                            break;
                        },
                    }
                }
                Some(almost_done) = about_to_finish.next() => {
                    if almost_done {
                        if let Some(url) = self.prep_next_track().await {
                            next_track_tx.send(url).expect("failed to send next track url");
                        }
                    }
                }
                Some(action) = actions.next() => {
                    match action {
                        Action::JumpBackward => self.jump_backward().await,
                        Action::JumpForward => self.jump_forward().await,
                        Action::Next => self.skip(SkipDirection::Forward,None).await.expect("failed to skip forward"),
                        Action::Pause => self.pause(),
                        Action::Play => self.play(),
                        Action::PlayPause => self.play_pause(),
                        Action::Previous => self.skip(SkipDirection::Backward,None).await.expect("failed to skip backward"),
                        Action::Stop => self.stop(),
                        Action::PlayAlbum { album_id } => {
                            if let Ok(album) = self.client.album(album_id).await {
                                self.play_album(album, None).await;
                            }
                        },
                        Action::PlayTrack { track_id } => {
                            if let Ok(track) = self.client.track(track_id).await {
                                self.play_track(track, None).await;
                            }
                        },
                        Action::PlayUri { uri } => self.play_uri(uri, Some(self.client.quality())).await,
                        Action::PlayPlaylist { playlist_id } => {
                            if let Ok(playlist) = self.client.playlist(playlist_id).await {
                                self.play_playlist(playlist, Some(self.client.quality())).await
                            }
                        },
                        Action::Quit => self.state.lock().await.quit(),
                        Action::SkipTo { num, direction } => self.skip(direction, Some(num)).await.expect("failed to skip to track"),
                        Action::SkipToById { track_id } => self.skip_to_by_id(track_id).await.expect("failed to skip to track"),
                    }
                }
                Some(msg) = messages.next() => {
                    match msg.view() {
                        MessageView::Eos(_) => {
                            debug!("END OF STREAM");

                            self.stop();
                            self.state.lock().await.quit();
                            break;
                        },
                        MessageView::StreamStart(_) => {
                            self.dbus_metadata_changed().await;
                        }
                        MessageView::DurationChanged(_) => {
                            if let Some(duration) = self.duration() {
                                debug!("duration changed");
                                let mut state = self.state.lock().await;
                                state.set_duration(duration);
                            }

                        }
                        MessageView::Buffering(buffering) => {
                            let percent = buffering.percent();
                            let mut state = self.state.lock().await;

                            if !self.is_buffering && percent < 100 {
                                debug!("buffering {}%", percent);
                                state.set_buffering(true);
                                self.is_buffering = true;
                            } else if self.is_buffering && percent > 99 {
                                state.set_buffering(false);
                                self.is_buffering = false;
                            }
                        }
                        MessageView::AsyncDone(_) => {
                            // If the player is resuming from a previous session,
                            // seek to the last position saved to the state.
                            // if self.resume {
                            //     if let Some(position) = self.db.get::<String, ClockValue>(StateKey::Player(PlayerKey::Position)).await {
                            //         let mut state = self.state.write().await;
                            //         state.set_resume(false);

                            //         self.seek(position, None).await;
                            //     }
                            // }
                        }
                        MessageView::StateChanged(state_changed) => {
                            if state_changed
                                .src()
                                .map(|s| s == self.playbin)
                                .unwrap_or(false)
                            {
                                let current_state = state_changed
                                    .current()
                                    .to_value()
                                    .get::<GstState>()
                                    .unwrap();

                                let iface_ref = self.player_iface().await;
                                let iface = iface_ref.get_mut().await;

                                match current_state {
                                    GstState::Playing => {
                                        debug!("player state changed to Playing");

                                        let mut state = self.state.lock().await;
                                        state.set_status(gstreamer::State::Playing.into());
                                        drop(state);

                                        iface
                                            .playback_status_changed(iface_ref.signal_context())
                                            .await
                                            .expect("failed");

                                    }
                                    GstState::Paused => {
                                        debug!("player state changed to Paused");
                                        let mut state = self.state.lock().await;
                                        state.set_status(gstreamer::State::Paused.into());
                                        drop(state);

                                        iface
                                            .playback_status_changed(iface_ref.signal_context())
                                            .await
                                            .expect("failed");
                                    }
                                    GstState::Ready => {
                                        debug!("player state changed to Ready");
                                        let mut state = self.state.lock().await;
                                        state.set_status(gstreamer::State::Ready.into());
                                        drop(state);

                                        iface
                                            .playback_status_changed(iface_ref.signal_context())
                                            .await
                                            .expect("failed");

                                    }
                                    GstState::VoidPending => {
                                        debug!("player state changed to VoidPending");
                                        let mut state = self.state.lock().await;
                                        state.set_status(gstreamer::State::VoidPending.into());
                                        drop(state);

                                        iface
                                            .playback_status_changed(iface_ref.signal_context())
                                            .await
                                            .expect("failed");

                                    },
                                    GstState::Null => {
                                        debug!("player state changed to Null");
                                        let mut state = self.state.lock().await;
                                        state.set_status(gstreamer::State::Null.into());
                                        drop(state);

                                        iface
                                            .playback_status_changed(iface_ref.signal_context())
                                            .await
                                            .expect("failed");
                                    },
                                    _ => (),

                                }
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
    }
    /// Inserts the most recent position, duration and progress values into the state
    /// at a set interval.
    async fn clock_loop(&self) {
        let mut quit_receiver = self.state.lock().await.quitter();

        loop {
            if let Ok(quit) = quit_receiver.try_recv() {
                if quit {
                    debug!("quitting");
                    break;
                }
            }
            if self.current_state() != GstState::VoidPending.into()
                || self.current_state() != GstState::Null.into()
            {
                if let (Some(position), Some(duration)) = (self.position(), self.duration()) {
                    let duration = duration.inner_clocktime();
                    let position = position.inner_clocktime();

                    let remaining = duration - position;
                    let progress = position.seconds() as f64 / duration.seconds() as f64;

                    let mut state = self.state.lock().await;
                    state.set_position(position.into());
                    state.set_current_progress(progress.into());
                    state.set_duration_remaining(remaining.into());
                    state.set_duration(duration.into());
                }

                std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
            }
        }
    }
    async fn dbus_track_list_replaced_signal(&self, tracks: Vec<String>, current: String) {
        debug!("replacing dbus tracklist");
        let object_server = self.connection.object_server();

        let iface_ref = object_server
            .interface::<_, MprisTrackList>("/org/mpris/MediaPlayer2")
            .await
            .expect("failed to get object server");

        MprisTrackList::track_list_replaced(iface_ref.signal_context(), tracks, current)
            .await
            .expect("failed to send track list replaced signal");
    }
    async fn player_iface(&self) -> zbus::InterfaceRef<MprisPlayer> {
        let object_server = self.connection.object_server();

        object_server
            .interface::<_, MprisPlayer>("/org/mpris/MediaPlayer2")
            .await
            .expect("failed to get object server")
    }
    async fn dbus_seeked_signal(&self, position: ClockValue) {
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
    async fn prep_next_track(&self) -> Option<String> {
        let mut state = self.state.lock().await;

        if let Some(mut next_track) = state.skip_track(None, SkipDirection::Forward).await {
            //self.dbus_metadata_changed().await;
            debug!("received new track, adding to player");
            if let Some(next_playlist_track_url) = next_track.track_url {
                Some(next_playlist_track_url.url)
            } else {
                None
            }
        } else {
            debug!("no more tracks left");
            None
        }
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
