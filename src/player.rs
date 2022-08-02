use crate::{
    mpris::{new_mpris, new_mpris_player},
    qobuz::{client::Client, Album, PlaylistTrack, Track, TrackURL},
    state::{
        app::{AppKey, AppState, PlayerKey},
        ClockValue, FloatValue, StatusValue,
    },
};
use clap::ValueEnum;
use flume::{Receiver, Sender};
use futures::prelude::*;
use gst::{glib, ClockTime, Element, MessageView, SeekFlags, State as GstState};
use gstreamer::{self as gst, prelude::*};
use hifi_rs::REFRESH_RESOLUTION;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sled::IVec;
use std::{
    collections::{vec_deque::IntoIter, VecDeque},
    fmt::Display,
    sync::Arc,
    time::Duration,
};
use tokio::{
    select,
    sync::broadcast::{self, Receiver as BroadcastReceiver, Sender as BroadcastSender},
};
use zbus::ConnectionBuilder;

/// The audio quality as defined by the Qobuz API.
#[derive(Clone, Debug, Serialize, Deserialize, ValueEnum)]
pub enum AudioQuality {
    Mp3 = 5,
    CD = 6,
    HIFI96 = 7,
    HIFI192 = 27,
}

impl Display for AudioQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.clone() as u32))
    }
}

impl From<IVec> for AudioQuality {
    fn from(ivec: IVec) -> Self {
        let deserialized: AudioQuality =
            bincode::deserialize(&ivec).expect("ERROR: failed to deserialize audio quality.");

        deserialized
    }
}

/// Signal is a sender and receiver in a single struct.
#[derive(Debug, Clone)]
struct Signal<T> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

/// Create a new signal.
fn new_signal<T>(_: T, size: i32) -> Signal<T> {
    let (sender, receiver) = flume::bounded::<T>(size.try_into().unwrap());
    Signal { sender, receiver }
}

/// A player handles playing media to a device.
#[derive(Debug, Clone)]
pub struct Player {
    /// Used to broadcast the player state out to other components.
    playbin: Element,
    /// List of tracks that will play.
    playlist: Arc<RwLock<Playlist>>,
    /// List of tracks that have played.
    playlist_previous: Arc<RwLock<Playlist>>,
    /// The app state to save player inforamtion into.
    state: AppState,
    /// Request URLs for tracks.
    url_request: Signal<PlaylistTrack>,
    /// Receive URLs for tracks
    url_return: Signal<PlaylistTrack>,
    broadcast_sender: BroadcastSender<AppState>,
}

/// A playlist is a list of tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist(VecDeque<PlaylistTrack>);

impl From<IVec> for Playlist {
    fn from(ivec: IVec) -> Self {
        let deserialized: Playlist =
            bincode::deserialize(&ivec).expect("failed to deserialize status value");

        deserialized
    }
}

#[allow(dead_code)]
impl Playlist {
    pub fn new() -> Playlist {
        Playlist(VecDeque::new())
    }

    pub fn into_iter(self) -> IntoIter<PlaylistTrack> {
        self.0.into_iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn front(&self) -> Option<&PlaylistTrack> {
        self.0.front()
    }

    pub fn back(&self) -> Option<&PlaylistTrack> {
        self.0.back()
    }

    pub fn pop_front(&mut self) -> Option<PlaylistTrack> {
        self.0.pop_front()
    }

    pub fn pop_back(&mut self) -> Option<PlaylistTrack> {
        self.0.pop_back()
    }

    pub fn push_front(&mut self, track: PlaylistTrack) {
        self.0.push_front(track)
    }

    pub fn push_back(&mut self, track: PlaylistTrack) {
        self.0.push_back(track)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub fn new(state: AppState) -> (Player, BroadcastReceiver<AppState>) {
    gst::init().expect("Couldn't initialize Gstreamer");
    let playbin = gst::ElementFactory::make("playbin", None).unwrap();
    let url_request = new_signal(PlaylistTrack::default(), 1);
    let url_return = new_signal(PlaylistTrack::default(), 1);
    let (broadcast_sender, broadcast_receiver) = broadcast::channel::<AppState>(1);

    (
        Player {
            playbin,
            playlist: Arc::new(RwLock::new(Playlist::new())),
            playlist_previous: Arc::new(RwLock::new(Playlist::new())),
            state,
            url_request,
            url_return,
            broadcast_sender,
        },
        broadcast_receiver,
    )
}

#[allow(dead_code)]
impl Player {
    /// Retreive the current app state from the player.
    pub fn app_state(&self) -> AppState {
        self.clone().state
    }
    /// Retreive the active playlist.
    pub fn playlist(&self) -> Arc<RwLock<Playlist>> {
        self.playlist.clone()
    }
    /// Set the active playlist.
    pub fn set_playlist(&mut self, playlist: Playlist) {
        self.playlist = Arc::new(RwLock::new(playlist));
    }
    /// Retreive the playlist of tracks played previously.
    pub fn prev_playlist(&self) -> Arc<RwLock<Playlist>> {
        self.playlist_previous.clone()
    }
    /// Set the previous playlist.
    pub fn set_prev_playlist(&mut self, playlist: Playlist) {
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
    /// Is the player in a VoidPending state?
    pub fn is_void(&self) -> bool {
        self.playbin.current_state() == gst::State::VoidPending
    }
    /// Is the player stopped?
    pub fn is_stopped(&self) -> bool {
        self.playbin.current_state() == gst::State::Null
    }
    /// Is the player ready?
    pub fn is_ready(&self) -> bool {
        self.playbin.current_state() == gst::State::Ready
    }
    /// Retreive the current Gstreamer state of the player.
    pub fn current_state(&self) -> GstState {
        self.playbin.current_state()
    }
    /// Retreive the current position from the player.
    pub fn position(&self) -> ClockTime {
        if let Some(position) = self.playbin.query_position::<ClockTime>() {
            position
        } else {
            ClockTime::default()
        }
    }
    /// Seek to a specified time in the current track.
    pub fn seek(&self, time: ClockValue, flag: SeekFlags) {
        match self.playbin.seek_simple(flag, time.inner_clocktime()) {
            Ok(_) => (),
            Err(error) => {
                error!("{}", error.message);
            }
        }
    }
    /// Retreive the current track's duration from the player.
    pub fn duration(&self) -> ClockTime {
        if let Some(duration) = self.playbin.query_duration::<ClockTime>() {
            duration
        } else {
            ClockTime::default()
        }
    }
    /// Jump forward in the currently playing track +10 seconds.
    pub fn jump_forward(&self) {
        if !self.is_playing() && !self.is_paused() {
            return;
        }
        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            if let Some(duration) = self.playbin.query_duration::<ClockTime>() {
                let ten_seconds = ClockTime::from_seconds(10);
                let next_position = current_position + ten_seconds;

                if next_position < duration {
                    match self.playbin.seek_simple(SeekFlags::FLUSH, next_position) {
                        Ok(_) => (),
                        Err(error) => {
                            error!("{:?}", error);
                        }
                    }
                } else {
                    match self.playbin.seek_simple(SeekFlags::FLUSH, duration) {
                        Ok(_) => (),
                        Err(error) => {
                            error!("{:?}", error);
                        }
                    }
                }
            }
        }
    }
    /// Jump forward in the currently playing track +10 seconds.
    pub fn jump_backward(&self) {
        if !self.is_playing() && !self.is_paused() {
            return;
        }
        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            if current_position.seconds() < 10 {
                match self
                    .playbin
                    .seek_simple(SeekFlags::FLUSH, ClockTime::default())
                {
                    Ok(_) => (),
                    Err(error) => {
                        error!("{:?}", error);
                    }
                }
            } else {
                let ten_seconds = ClockTime::from_seconds(10);
                match self
                    .playbin
                    .seek_simple(SeekFlags::FLUSH, current_position - ten_seconds)
                {
                    Ok(_) => (),
                    Err(error) => {
                        error!("{:?}", error);
                    }
                }
            }
        }
    }
    /// Skip forward to the next track in the playlist.
    pub fn skip_forward(&self, num: Option<usize>) {
        let mut prev_playlist = self.playlist_previous.write();

        if let Some(previous_track) = &self
            .state
            .player
            .get::<String, PlaylistTrack>(AppKey::Player(PlayerKey::NextUp))
        {
            prev_playlist.push_back(previous_track.clone());
        }

        if let Some(number) = num {
            // Grab all of the tracks, up to the next one to play.
            let mut playlist = self.playlist.write();
            let mut skipped_tracks = playlist
                .0
                .drain(..number - 1)
                .collect::<VecDeque<PlaylistTrack>>();

            prev_playlist.0.append(&mut skipped_tracks);
        }

        let mut playlist = self.playlist.write();
        if let Some(mut next_track) = playlist.pop_front() {
            debug!("skipping forward to next track");
            self.ready();

            debug!("receiving track url");
            next_track = self.fetch_track_url(next_track);

            self.state.player.insert::<String, PlaylistTrack>(
                AppKey::Player(PlayerKey::NextUp),
                next_track.clone(),
            );

            self.state
                .player
                .insert::<String, Playlist>(AppKey::Player(PlayerKey::Playlist), playlist.clone());

            self.state.player.insert::<String, Playlist>(
                AppKey::Player(PlayerKey::PreviousPlaylist),
                prev_playlist.clone(),
            );

            let track_url = next_track.track_url.expect("missing track url");
            self.playbin.set_property("uri", Some(track_url.url));
            self.play();
        }
    }
    /// Skip backwards by playing the first track in previous track playlist.
    pub fn skip_backward(&self, num: Option<usize>) {
        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            let one_second = ClockTime::from_seconds(1);

            if current_position > one_second && num.is_none() {
                debug!("current track position >1s, seeking to start of track");
                self.playbin
                    .seek_simple(SeekFlags::FLUSH, ClockTime::default())
                    .expect("failed to seek");
                return;
            }
        }

        let mut playlist = self.playlist.write();

        if let Some(previous_track) = &self
            .state
            .player
            .get::<String, PlaylistTrack>(AppKey::Player(PlayerKey::NextUp))
        {
            playlist.push_front(previous_track.clone());
        }

        if let Some(number) = num {
            // Grab all of the tracks, up to the next one to play.
            let diff = number - playlist.len();

            let mut prev_playlist = self.playlist_previous.write();
            let skipped_tracks = prev_playlist
                .0
                .drain(diff + 1..)
                .rev()
                .collect::<VecDeque<PlaylistTrack>>();

            for track in skipped_tracks {
                playlist.push_front(track);
            }
        }

        let mut prev_playlist = self.playlist_previous.write();
        if let Some(mut prev_track) = prev_playlist.pop_back() {
            debug!("skipping backward to previous track");
            self.ready();

            prev_track = self.fetch_track_url(prev_track);

            self.state.player.insert::<String, PlaylistTrack>(
                AppKey::Player(PlayerKey::NextUp),
                prev_track.clone(),
            );

            self.state
                .player
                .insert::<String, Playlist>(AppKey::Player(PlayerKey::Playlist), playlist.clone());
            self.state.player.insert::<String, Playlist>(
                AppKey::Player(PlayerKey::PreviousPlaylist),
                prev_playlist.clone(),
            );

            let track_url = prev_track.track_url.expect("missing track url");
            self.playbin.set_property("uri", Some(track_url.url));

            self.play();
        }
    }
    /// Skip to a specific track number in the combined playlist.
    pub fn skip_to(&self, track_number: usize) -> bool {
        if track_number <= self.playlist().read().len() {
            self.skip_forward(Some(track_number));
            true
        } else {
            self.skip_backward(Some(track_number));
            true
        }
    }
    /// Adds the most recent position and progress values to the state
    /// then broadcasts the state to listeners.
    async fn broadcast_loop(&self, mut quit_receiver: BroadcastReceiver<bool>) {
        loop {
            if let Ok(quit) = quit_receiver.try_recv() {
                if quit {
                    debug!("quitting");
                    break;
                }
            }
            if self.playbin.current_state() != GstState::VoidPending
                || self.playbin.current_state() != GstState::Null
            {
                let pos: Option<ClockTime> = self.playbin.query_position();
                let dur: Option<ClockTime> = self.playbin.query_duration();
                let state = self.state.clone();

                if let Some(position) = pos {
                    state.player.insert::<String, ClockValue>(
                        AppKey::Player(PlayerKey::Position),
                        position.into(),
                    );

                    if let Some(duration) = dur {
                        state.player.insert::<String, ClockValue>(
                            AppKey::Player(PlayerKey::Duration),
                            duration.into(),
                        );

                        if position >= ClockTime::from_seconds(0) && position <= duration {
                            let remaining = duration - position;
                            let progress = position.seconds() as f64 / duration.seconds() as f64;
                            state.player.insert::<String, FloatValue>(
                                AppKey::Player(PlayerKey::Progress),
                                progress.into(),
                            );
                            state.player.insert::<String, ClockValue>(
                                AppKey::Player(PlayerKey::DurationRemaining),
                                remaining.into(),
                            );
                        }
                    }
                }

                self.broadcast_sender
                    .send(state)
                    .expect("failed to broadcast app state");

                std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
            }
        }
    }
    /// Plays a single track.
    pub async fn play_track(&self, track: Track, quality: AudioQuality, client: Client) {
        let playlist_track = PlaylistTrack::new(track, Some(quality.clone()), None);
        self.playlist.write().push_back(playlist_track);
        self.start(quality, client).await;
    }
    /// Plays a full album.
    pub async fn play_album(&self, album: Album, quality: AudioQuality, client: Client) {
        if let Some(tracklist) = album.to_playlist_tracklist(quality.clone()) {
            debug!("creating playlist");
            for playlist_track in tracklist {
                self.playlist.write().push_back(playlist_track);
            }

            self.start(quality, client).await;
        }
    }
    /// Stats the player.
    async fn start(&self, quality: AudioQuality, mut client: Client) {
        let mut next_track = match self.playlist.write().pop_front() {
            Some(it) => it,
            _ => return,
        };
        let playbin = &self.playbin;

        if let Ok(track_url) = client
            .track_url(next_track.track.id, Some(quality.clone()), None)
            .await
        {
            playbin.set_property("uri", Some(track_url.url.as_str()));
            next_track.set_track_url(track_url);

            self.state.player.insert::<String, PlaylistTrack>(
                AppKey::Player(PlayerKey::NextUp),
                next_track.clone(),
            );

            self.state.player.insert::<String, Playlist>(
                AppKey::Player(PlayerKey::Playlist),
                self.playlist.read().clone(),
            );

            self.play();

            self.state.player.insert::<String, StatusValue>(
                AppKey::Player(PlayerKey::Status),
                gst::State::Playing.into(),
            );
        }
    }
    /// Sets up basic functionality for the player.
    pub async fn setup(&self, client: Client, resume: bool) {
        let mpris = new_mpris(self.clone());
        let mpris_player = new_mpris_player(self.clone());

        ConnectionBuilder::session()
            .unwrap()
            .name("org.mpris.MediaPlayer2.hifirs")
            .unwrap()
            .serve_at("/org/mpris/MediaPlayer2", mpris)
            .unwrap()
            .serve_at("/org/mpris/MediaPlayer2", mpris_player)
            .unwrap()
            .build()
            .await
            .expect("failed to attach to dbus");

        let cloned_self = self.clone();
        let quitter = self.app_state().quitter();
        tokio::spawn(async move {
            cloned_self.broadcast_loop(quitter).await;
        });

        let mut cloned_self = self.clone();
        tokio::spawn(async move {
            cloned_self.player_loop(client, resume).await;
        });

        let url_request = self.url_request.sender.clone();
        let url_return = self.url_return.receiver.clone();
        let playlist = self.playlist.clone();
        let prev_playlist = self.playlist_previous.clone();
        let state = self.state.clone();

        self.playbin
            .connect("about-to-finish", false, move |values| {
                debug!("about to finish");
                let cloned_state = state.clone();
                let playbin = values[0]
                    .get::<glib::Object>()
                    .expect("playbin \"about-to-finish\" signal values[0]");

                if let Some(next_track) = playlist.write().pop_front() {
                    debug!("received new track, adding to player");
                    url_request.send(next_track).unwrap();
                    if let Ok(next_playlist_track) = url_return.recv() {
                        if let Some(track_url) = next_playlist_track.clone().track_url {
                            if let Some(previous_track) = &cloned_state
                                .player
                                .get::<String, PlaylistTrack>(AppKey::Player(PlayerKey::NextUp))
                            {
                                prev_playlist.write().push_back(previous_track.clone());
                            }
                            cloned_state.player.insert::<String, PlaylistTrack>(
                                AppKey::Player(PlayerKey::NextUp),
                                next_playlist_track,
                            );
                            playbin.set_property("uri", Some(track_url.url));
                        }
                    }
                }

                None
            });
    }
    pub fn fetch_track_url(&self, track: PlaylistTrack) -> PlaylistTrack {
        let url_request = &self.url_request.sender;
        let url_return = &self.url_return.receiver;

        url_request
            .send(track)
            .expect("failed to send url to request");

        url_return.recv().expect("failed to get track url")
    }
    /// Handles messages from the player and takes necessary action.
    async fn player_loop(&mut self, mut client: Client, mut resume: bool) {
        let mut messages = self.playbin.bus().unwrap().stream();
        let mut url_request = self.url_request.receiver.stream();
        let mut quitter = self.state.quitter();

        loop {
            select! {
                Ok(quit) = quitter.recv() => {
                    if quit {
                        debug!("quitting");
                        break;
                    }
                }
                Some(mut playlist_track) = url_request.next() => {
                    if let Ok(track_url) = client.track_url(playlist_track.track.id,playlist_track.quality.clone(), None).await {
                        let with_track_url = playlist_track.set_track_url(track_url);

                        match self.url_return.sender.send_async(with_track_url).await {
                            Ok(_) => (),
                            Err(error) => {
                                error!("{:?}", error);
                            }
                        }
                    }
                }
                Some(msg) = messages.next() => {
                    match msg.view() {
                        MessageView::Eos(_) => {
                            debug!("END OF STREAM");

                            self.stop();
                            self.state.send_quit();
                            break;
                        },
                        MessageView::StreamStart(_) => {
                            let state = &mut self.state;

                            if let Some(next_track) = state.player.get::<String, PlaylistTrack>(AppKey::Player(PlayerKey::NextUp)) {
                               state.player.insert::<String, ClockValue>(AppKey::Player(PlayerKey::Duration),ClockTime::from_seconds(next_track.track.duration.try_into().unwrap()).into());
                            }
                        }
                        MessageView::AsyncDone(_) => {
                            if resume {
                                let state = &mut self.state;

                                if let Some(position) = state.player.get::<String, ClockValue>(AppKey::Player(PlayerKey::Position)) {
                                    self.seek(position, SeekFlags::FLUSH | SeekFlags::KEY_UNIT);
                                    resume = false;
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
                                        self.state.player.insert::<String, StatusValue>(AppKey::Player(PlayerKey::Status),GstState::Playing.into());
                                    }
                                    GstState::Paused => {
                                        debug!("player state changed to Paused");
                                        self.state.player.insert::<String, StatusValue>(AppKey::Player(PlayerKey::Status),GstState::Paused.into());
                                    }
                                    GstState::Ready => {
                                        debug!("player state changed to Ready");
                                        self.state.player.insert::<String, StatusValue>(AppKey::Player(PlayerKey::Status),GstState::Ready.into());
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
}
