use crate::{
    action, get_player, mpris,
    qobuz::{
        album::Album,
        client::Client,
        track::{PlaylistTrack, Track},
        TrackURL,
    },
    state::{
        app::{AppState, PlayerKey, StateKey},
        AudioQuality, ClockValue, FloatValue, PlaylistValue, StatusValue,
    },
    REFRESH_RESOLUTION,
};
use flume::{Receiver, Sender};
use futures::prelude::*;
use gst::{bus::BusStream, glib, ClockTime, Element, MessageView, SeekFlags, State as GstState};
use gstreamer::{self as gst, prelude::*};
use snafu::prelude::*;
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::{select, sync::RwLock};

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
    SkipTo { num: usize },
    JumpForward,
    JumpBackward,
    PlayAlbum { album: Box<Album> },
    Clear,
}

/// A player handles playing media to a device.
#[derive(Debug, Clone)]
pub struct Player {
    /// Used to broadcast the player state out to other components.
    playbin: Element,
    /// List of tracks that will play.
    playlist: Arc<RwLock<PlaylistValue>>,
    /// List of tracks that have played.
    playlist_previous: Arc<RwLock<PlaylistValue>>,
    /// The app state to save player inforamtion into.
    app_state: AppState,
    /// Qobuz client
    client: Client,
    controls: Controls,
    is_buffering: bool,
}

pub async fn new(app_state: AppState, client: Client, resume: bool) -> Player {
    gst::init().expect("Couldn't initialize Gstreamer");
    let playbin = gst::ElementFactory::make("playbin", None).expect("failed to create gst element");
    let controls = Controls::new(app_state.clone());
    let playlist = Arc::new(RwLock::new(PlaylistValue::new()));
    let playlist_previous = Arc::new(RwLock::new(PlaylistValue::new()));

    mpris::init(controls.clone()).await;

    let (about_to_finish_tx, about_to_finish_rx) = flume::bounded::<bool>(1);
    let (next_track_tx, next_track_rx) = flume::bounded::<String>(1);

    // Connects to the `about-to-finish` signal so the player
    // can setup the next track to play. Enables gapless playback.
    playbin.connect("about-to-finish", false, move |values| {
        debug!("about to finish");
        about_to_finish_tx
            .send(true)
            .expect("failed to send about to finish message");

        let next_track_url = next_track_rx
            .recv()
            .expect("failed to receive next track url");

        let playbin = values[0]
            .get::<glib::Object>()
            .expect("playbin \"about-to-finish\" signal values[0]");

        playbin.set_property("uri", Some(next_track_url));

        None
    });

    let mut player = Player {
        client,
        playbin,
        playlist,
        playlist_previous,
        app_state,
        controls,
        is_buffering: false,
    };

    if resume {
        player.resume().await.expect("failed to resume");
    }

    let p = player.clone();
    tokio::spawn(async move {
        p.clock_loop().await;
    });

    let mut p = player.clone();
    tokio::spawn(async move {
        p.player_loop(resume, about_to_finish_rx, next_track_tx)
            .await;
    });

    player
}

impl Player {
    /// Retreive the current app state from the player.
    pub fn app_state(&self) -> &AppState {
        &self.app_state
    }
    /// Retreive the active playlist.
    pub fn playlist(&self) -> Arc<RwLock<PlaylistValue>> {
        self.playlist.clone()
    }
    /// Set the active playlist.
    pub fn set_playlist(&mut self, playlist: PlaylistValue) {
        self.playlist = Arc::new(RwLock::new(playlist));
    }
    /// Set the previous playlist.
    pub fn set_prev_playlist(&mut self, playlist: PlaylistValue) {
        self.playlist_previous = Arc::new(RwLock::new(playlist));
    }
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
    pub fn seek(&self, time: ClockValue, flags: Option<SeekFlags>) -> Result<()> {
        let flags = if let Some(flags) = flags {
            flags
        } else {
            SeekFlags::FLUSH | SeekFlags::KEY_UNIT
        };

        match self.playbin.seek_simple(flags, time.inner_clocktime()) {
            Ok(_) => {
                self.app_state
                    .player
                    .insert::<String, ClockValue>(StateKey::Player(PlayerKey::Position), time);

                Ok(())
            }
            Err(error) => {
                error!("{}", error.message);
                Err(Error::Seek)
            }
        }
    }
    pub async fn resume(&mut self) -> Result<()> {
        let tree = self.app_state.player.clone();

        if let (Some(playlist), Some(next_up)) = (
            get_player!(PlayerKey::Playlist, tree, PlaylistValue),
            get_player!(PlayerKey::NextUp, tree, PlaylistTrack),
        ) {
            let next_track = self.attach_track_url(next_up).await?;

            if let Some(track_url) = next_track.track_url {
                self.set_playlist(playlist);
                self.set_uri(track_url);

                if let Some(prev_playlist) =
                    get_player!(PlayerKey::PreviousPlaylist, tree, PlaylistValue)
                {
                    self.set_prev_playlist(prev_playlist);

                    self.pause();
                }
            }
            Ok(())
        } else {
            Err(Error::Session)
        }
    }
    /// Retreive controls for the player.
    pub fn controls(&self) -> Controls {
        self.controls.clone()
    }
    /// Jump forward in the currently playing track +10 seconds.
    pub fn jump_forward(&self) {
        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            if let Some(duration) = self.playbin.query_duration::<ClockTime>() {
                let ten_seconds = ClockTime::from_seconds(10);
                let next_position = current_position + ten_seconds;

                if next_position < duration {
                    match self.seek(next_position.into(), None) {
                        Ok(_) => (),
                        Err(error) => {
                            error!("{:?}", error);
                        }
                    }
                } else {
                    match self.seek(duration.into(), None) {
                        Ok(_) => (),
                        Err(error) => {
                            error!("{:?}", error);
                        }
                    }
                }
            }
        }
    }
    /// Jump forward in the currently playing track -10 seconds.
    pub fn jump_backward(&self) {
        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            if current_position.seconds() < 10 {
                match self.seek(ClockTime::default().into(), None) {
                    Ok(_) => self.app_state().player.insert::<String, ClockValue>(
                        StateKey::Player(PlayerKey::Position),
                        ClockTime::default().into(),
                    ),
                    Err(error) => {
                        error!("{:?}", error);
                        self.app_state().player.insert::<String, ClockValue>(
                            StateKey::Player(PlayerKey::Position),
                            current_position.into(),
                        )
                    }
                }
            } else {
                let ten_seconds = ClockTime::from_seconds(10);
                let seek_position = current_position - ten_seconds;
                match self.seek(seek_position.into(), None) {
                    Ok(_) => self.app_state().player.insert::<String, ClockValue>(
                        StateKey::Player(PlayerKey::Position),
                        seek_position.into(),
                    ),
                    Err(error) => {
                        error!("{:?}", error);
                        self.app_state().player.insert::<String, ClockValue>(
                            StateKey::Player(PlayerKey::Position),
                            current_position.into(),
                        )
                    }
                }
            }
        }
    }
    /// Skip forward to the next track in the playlist.
    pub async fn skip_forward(&self, num: Option<usize>) -> Result<()> {
        let tree = self.app_state.player.clone();

        let mut playlist = self.playlist.write().await;
        let mut prev_playlist = self.playlist_previous.write().await;

        if let Some(previous_track) = get_player!(PlayerKey::NextUp, tree, PlaylistTrack) {
            prev_playlist.push_back(previous_track);
        }

        if let Some(number) = num {
            // Grab all of the tracks, up to the next one to play.
            prev_playlist.append(
                playlist
                    .drain(..number)
                    .collect::<VecDeque<PlaylistTrack>>(),
            );
        }

        if let Some(mut next_track_to_play) = playlist.pop_front() {
            debug!("fetching url for next track");

            next_track_to_play = self.attach_track_url(next_track_to_play).await?;

            if let Some(track_url) = next_track_to_play.clone().track_url {
                debug!("skipping forward to next track");
                self.ready();

                self.app_state.player.insert::<String, PlaylistTrack>(
                    StateKey::Player(PlayerKey::NextUp),
                    next_track_to_play,
                );

                self.app_state.player.insert::<String, PlaylistValue>(
                    StateKey::Player(PlayerKey::Playlist),
                    playlist.clone(),
                );

                self.app_state.player.insert::<String, PlaylistValue>(
                    StateKey::Player(PlayerKey::PreviousPlaylist),
                    prev_playlist.clone(),
                );

                self.playbin.set_property("uri", Some(track_url.url));
                self.play();
            }
        }
        Ok(())
    }
    /// Skip backwards by playing the first track in previous track playlist.
    pub async fn skip_backward(&self, num: Option<usize>) -> Result<()> {
        // If the track is greater than 1 second into playing,
        // then we just want to go back to the beginning.
        // If triggered again within a second after playing,
        // it will skip to the previous track.
        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            let one_second = ClockTime::from_seconds(1);

            if current_position > one_second && num.is_none() {
                debug!("current track position >1s, seeking to start of track");
                self.seek(ClockTime::default().into(), None)
                    .expect("failed to seek");

                return Ok(());
            }
        }

        let mut prev_playlist = self.playlist_previous.write().await;
        let mut playlist = self.playlist.write().await;
        let tree = &self.app_state.player;

        if let Some(previously_played_track) = get_player!(PlayerKey::NextUp, tree, PlaylistTrack) {
            playlist.push_front(previously_played_track);
        }

        if let Some(number) = num {
            // Grab all of the tracks, up to the next one to play,
            // inlcuding the currently playing track from above.
            let diff = number + 1 - playlist.len();
            let skipped_tracks = prev_playlist
                .drain(diff + 1..)
                .rev()
                .collect::<VecDeque<PlaylistTrack>>();

            for track in skipped_tracks {
                playlist.push_front(track);
            }
        }

        if let Some(mut next_track_to_play) = prev_playlist.pop_back() {
            next_track_to_play = self.attach_track_url(next_track_to_play).await?;

            if let Some(track_url) = next_track_to_play.clone().track_url {
                debug!("skipping backward to previous track");
                self.ready();

                self.app_state.player.insert::<String, PlaylistTrack>(
                    StateKey::Player(PlayerKey::NextUp),
                    next_track_to_play,
                );

                self.app_state.player.insert::<String, PlaylistValue>(
                    StateKey::Player(PlayerKey::Playlist),
                    playlist.clone(),
                );
                self.app_state.player.insert::<String, PlaylistValue>(
                    StateKey::Player(PlayerKey::PreviousPlaylist),
                    prev_playlist.clone(),
                );

                self.playbin.set_property("uri", Some(track_url.url));
                self.play();
            }
        }

        Ok(())
    }
    /// Skip to a specific track number in the current playlist.
    pub async fn skip_to(&self, track_number: usize) -> Result<()> {
        if track_number < self.playlist().read().await.len() {
            debug!("skipping forward to track number {}", track_number);
            self.skip_forward(Some(track_number)).await
        } else {
            debug!("skipping backward to track number {}", track_number);
            self.skip_backward(Some(track_number)).await
        }
    }
    /// Plays a single track.
    pub async fn play_track(&self, track: Track, quality: AudioQuality) {
        let playlist_track = PlaylistTrack::new(track, Some(quality.clone()), None);
        self.playlist.write().await.push_back(playlist_track);
        self.start(quality).await;
    }
    /// Plays a full album.
    pub async fn play_album(&self, album: Album, quality: AudioQuality) {
        if let Some(tracklist) = album.to_playlist_tracklist(quality.clone()) {
            debug!("creating playlist");
            for playlist_track in tracklist {
                self.playlist.write().await.push_back(playlist_track);
            }

            self.start(quality).await;
        }
    }
    /// Stats the player.
    async fn start(&self, quality: AudioQuality) {
        let mut next_track = match self.playlist.write().await.pop_front() {
            Some(it) => it,
            _ => return,
        };
        let playbin = &self.playbin;

        if let Ok(track_url) = self
            .client
            .track_url(next_track.track.id, Some(quality.clone()), None)
            .await
        {
            playbin.set_property("uri", Some(track_url.url.as_str()));
            next_track.set_track_url(track_url);

            self.app_state.player.insert::<String, PlaylistTrack>(
                StateKey::Player(PlayerKey::NextUp),
                next_track.clone(),
            );

            self.app_state.player.insert::<String, PlaylistValue>(
                StateKey::Player(PlayerKey::Playlist),
                self.playlist.read().await.clone(),
            );

            self.play();

            self.app_state.player.insert::<String, StatusValue>(
                StateKey::Player(PlayerKey::Status),
                gst::State::Playing.into(),
            );
        }
    }
    /// Handles messages from the player and takes necessary action.
    async fn player_loop<'p>(
        &mut self,
        resume: bool,
        about_to_finish_rx: Receiver<bool>,
        next_track_tx: Sender<String>,
    ) {
        let action_rx = self.controls.action_receiver();
        let mut messages = self.message_stream().await;
        let mut quitter = self.app_state.quitter();
        let mut actions = action_rx.stream();
        let mut about_to_finish = about_to_finish_rx.stream();
        let mut resume = resume;

        loop {
            select! {
                Ok(quit) = quitter.recv() => {
                    if quit {
                        debug!("quitting");
                        break;
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
                        Action::Play => self.play(),
                        Action::Pause => self.pause(),
                        Action::PlayPause => self.play_pause(),
                        Action::Next => self.skip_forward(None).await.expect("failed to skip forward"),
                        Action::Previous => self.skip_backward(None).await.expect("failed to skip forward"),
                        Action::Stop => {
                            self.stop();
                            self.app_state.quit();
                        },
                        Action::SkipTo { num } => self.skip_to(num).await.expect("failed to skip to track"),
                        Action::JumpForward => self.jump_forward(),
                        Action::JumpBackward => self.jump_backward(),
                        Action::Clear => {
                            if self.is_playing() {
                                self.stop();
                            }

                            self.playlist.write().await.clear();
                            self.playlist_previous.write().await.clear();
                            self.app_state.player.clear();
                        }
                        Action::PlayAlbum { album } => {
                            let default_quality = self.client.default_quality.clone();

                            let client = self.client.clone();

                            let mut album = *album;
                            album.attach_tracks(client).await;

                            self.play_album(album, default_quality).await;
                        }
                    }
                }
                Some(msg) = messages.next() => {
                    match msg.view() {
                        MessageView::Eos(_) => {
                            debug!("END OF STREAM");

                            self.stop();
                            self.app_state.quit();
                            break;
                        },
                        MessageView::StreamStart(_) => {

                        }
                        MessageView::DurationChanged(_) => {
                            if let Some(duration) = self.duration() {
                                self.app_state.player.insert::<String, ClockValue>(StateKey::Player(PlayerKey::Duration),duration);
                            }

                        }
                        MessageView::Buffering(_) => {
                            debug!("buffering");
                            self.is_buffering = true;
                        }
                        MessageView::AsyncDone(_) => {
                            // If the player is resuming from a previous session,
                            // seek to the last position saved to the state.
                            if resume {
                                let tree = &self.app_state.player;

                                if let Some(position) = get_player!(PlayerKey::Position, tree, ClockValue) {
                                    resume = false;
                                    self.seek(position, None).expect("seek failure");

                                }
                            }

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

                                match current_state {
                                    GstState::Playing => {
                                        debug!("player state changed to Playing");
                                        self.is_buffering = false;
                                        self.app_state.player.insert::<String, StatusValue>(StateKey::Player(PlayerKey::Status),GstState::Playing.into());
                                    }
                                    GstState::Paused => {
                                        debug!("player state changed to Paused");
                                        self.is_buffering = false;
                                        self.app_state.player.insert::<String, StatusValue>(StateKey::Player(PlayerKey::Status),GstState::Paused.into());
                                    }
                                    GstState::Ready => {
                                        debug!("player state changed to Ready");
                                        self.is_buffering = false;
                                        self.app_state.player.insert::<String, StatusValue>(StateKey::Player(PlayerKey::Status),GstState::Ready.into());
                                    }
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
    async fn clock_loop<'p>(&self) {
        let mut quit_receiver = self.app_state.quitter();

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
                    self.app_state.player.insert::<String, ClockValue>(
                        StateKey::Player(PlayerKey::Position),
                        position.clone(),
                    );

                    self.app_state.player.insert::<String, ClockValue>(
                        StateKey::Player(PlayerKey::Duration),
                        duration.clone(),
                    );

                    if position >= ClockTime::from_seconds(0).into() && position <= duration {
                        let duration = duration.inner_clocktime();
                        let position = position.inner_clocktime();

                        let remaining = duration - position;
                        let progress = position.seconds() as f64 / duration.seconds() as f64;

                        self.app_state.player.insert::<String, FloatValue>(
                            StateKey::Player(PlayerKey::Progress),
                            progress.into(),
                        );
                        self.app_state.player.insert::<String, ClockValue>(
                            StateKey::Player(PlayerKey::DurationRemaining),
                            remaining.into(),
                        );
                    }
                }

                std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
            }
        }
    }
    /// Sets up basic functionality for the player.
    async fn prep_next_track(&self) -> Option<String> {
        let mut playlist = self.playlist.write().await;
        let mut prev_playlist = self.playlist_previous.write().await;

        if let Some(mut next_track) = playlist.pop_front() {
            debug!("received new track, adding to player");
            if let Ok(next_playlist_track_url) =
                self.client.track_url(next_track.track.id, None, None).await
            {
                let player_tree = self.app_state.player.clone();
                if let Some(previous_track) =
                    get_player!(PlayerKey::NextUp, player_tree, PlaylistTrack)
                {
                    prev_playlist.push_back(previous_track);
                }

                next_track.set_track_url(next_playlist_track_url.clone());

                self.app_state.player.insert::<String, PlaylistTrack>(
                    StateKey::Player(PlayerKey::NextUp),
                    next_track,
                );

                self.app_state.player.insert::<String, PlaylistValue>(
                    StateKey::Player(PlayerKey::Playlist),
                    playlist.clone(),
                );

                self.app_state.player.insert::<String, PlaylistValue>(
                    StateKey::Player(PlayerKey::PreviousPlaylist),
                    prev_playlist.clone(),
                );

                Some(next_playlist_track_url.url)
            } else {
                None
            }
        } else {
            None
        }
    }
    /// Attach a `TrackURL` to the given track.
    pub async fn attach_track_url(&self, mut track: PlaylistTrack) -> Result<PlaylistTrack> {
        if let Ok(track_url) = self.client.track_url(track.track.id, None, None).await {
            Ok(track.set_track_url(track_url))
        } else {
            Err(Error::TrackURL)
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
    state: AppState,
}

impl Controls {
    fn new(state: AppState) -> Controls {
        let (action_tx, action_rx) = flume::bounded::<Action>(1);

        Controls {
            action_rx,
            action_tx,
            state,
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
    pub async fn next(&self) {
        action!(self, Action::Next);
    }
    pub async fn previous(&self) {
        action!(self, Action::Previous);
    }
    pub async fn skip_to(&self, num: usize) {
        action!(self, Action::SkipTo { num });
    }
    pub async fn jump_forward(&self) {
        action!(self, Action::JumpForward);
    }
    pub async fn jump_backward(&self) {
        action!(self, Action::JumpBackward);
    }
    pub async fn play_album(&self, album: Album) {
        action!(
            self,
            Action::PlayAlbum {
                album: Box::new(album)
            }
        );
    }
    pub async fn clear(&self) {
        action!(self, Action::Clear);
    }
    pub async fn position(&self) -> Option<ClockValue> {
        let tree = &self.state.player;

        get_player!(PlayerKey::Position, tree, ClockValue)
    }
    pub async fn status(&self) -> Option<StatusValue> {
        let tree = &self.state.player;

        get_player!(PlayerKey::Status, tree, StatusValue)
    }
    pub async fn currently_playing_track(&self) -> Option<PlaylistTrack> {
        let tree = &self.state.player;

        get_player!(PlayerKey::NextUp, tree, PlaylistTrack)
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
