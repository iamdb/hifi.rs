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

fn new_signal<T>(_: T, size: i32) -> Signal<T> {
    let (sender, receiver) = flume::bounded::<T>(size.try_into().unwrap());
    Signal { sender, receiver }
}

#[derive(Debug, Clone)]
pub struct Player {
    broadcast_sender: BroadcastSender<AppState>,
    playbin: Element,
    playlist: Arc<RwLock<Playlist>>,
    playlist_previous: Arc<RwLock<Playlist>>,
    state: AppState,
    url_request: Signal<PlaylistTrack>,
    url_return: Signal<PlaylistTrack>,
}

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
    let (broadcast_sender, broadcast_receiver) = broadcast::channel::<AppState>(10);

    (
        Player {
            broadcast_sender,
            playbin,
            playlist: Arc::new(RwLock::new(Playlist::new())),
            playlist_previous: Arc::new(RwLock::new(Playlist::new())),
            state,
            url_request,
            url_return,
        },
        broadcast_receiver,
    )
}

#[allow(dead_code)]
impl Player {
    pub fn app_state(&self) -> AppState {
        self.clone().state
    }
    pub fn playlist(&self) -> Arc<RwLock<Playlist>> {
        self.playlist.clone()
    }
    pub fn set_playlist(&mut self, playlist: Playlist) {
        self.playlist = Arc::new(RwLock::new(playlist));
    }
    pub fn prev_playlist(&self) -> Arc<RwLock<Playlist>> {
        self.playlist_previous.clone()
    }
    pub fn set_prev_playlist(&mut self, playlist: Playlist) {
        self.playlist_previous = Arc::new(RwLock::new(playlist));
    }
    pub fn play(&self) {
        self.playbin
            .set_state(gst::State::Playing)
            .expect("Unable to set the pipeline to the `Playing` state");
    }
    pub fn pause(&self) {
        self.playbin
            .set_state(gst::State::Paused)
            .expect("Unable to set the pipeline to the `Paused` state");
    }
    pub fn ready(&self) {
        self.playbin
            .set_state(gst::State::Ready)
            .expect("Unable to set the pipeline to the `Ready` state");
    }
    pub fn stop(&self) {
        self.playbin
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");
    }
    pub fn set_uri(&self, track_url: TrackURL) {
        self.playbin
            .set_property("uri", Some(track_url.url.as_str()));
    }
    pub fn is_paused(&self) -> bool {
        self.playbin.current_state() == gst::State::Paused
    }
    pub fn is_playing(&self) -> bool {
        self.playbin.current_state() == gst::State::Playing
    }
    pub fn is_void(&self) -> bool {
        self.playbin.current_state() == gst::State::VoidPending
    }
    pub fn is_stopped(&self) -> bool {
        self.playbin.current_state() == gst::State::Null
    }
    pub fn is_ready(&self) -> bool {
        self.playbin.current_state() == gst::State::Ready
    }
    pub fn current_state(&self) -> GstState {
        self.playbin.current_state()
    }
    pub fn position(&self) -> ClockTime {
        if let Some(position) = self.playbin.query_position::<ClockTime>() {
            position
        } else {
            ClockTime::default()
        }
    }
    pub fn seek(&self, time: ClockValue, flag: SeekFlags) {
        match self.playbin.seek_simple(flag, time.clock_time()) {
            Ok(_) => (),
            Err(error) => {
                error!("{}", error.message);
            }
        }
    }
    pub fn duration(&self) -> ClockTime {
        if let Some(duration) = self.playbin.query_duration::<ClockTime>() {
            duration
        } else {
            ClockTime::default()
        }
    }
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
    pub fn skip_forward(&self, num: Option<usize>) {
        let url_request = &self.url_request.sender;
        let url_return = &self.url_return.receiver;
        let mut playlist = self.playlist.write();
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
            let mut skipped_tracks = playlist
                .0
                .drain(..number - 1)
                .collect::<VecDeque<PlaylistTrack>>();

            prev_playlist.0.append(&mut skipped_tracks);
        }

        if let Some(next_track) = playlist.pop_front() {
            debug!("skipping forward to next track");

            url_request
                .send(next_track)
                .expect("failed to send url to request");

            if let Ok(next_playlist_track) = url_return.recv() {
                if let Some(track_url) = next_playlist_track.clone().track_url {
                    self.ready();

                    self.state.player.insert::<String, PlaylistTrack>(
                        AppKey::Player(PlayerKey::NextUp),
                        next_playlist_track,
                    );

                    self.state.player.insert::<String, Playlist>(
                        AppKey::Player(PlayerKey::Playlist),
                        playlist.clone(),
                    );
                    self.state.player.insert::<String, Playlist>(
                        AppKey::Player(PlayerKey::PreviousPlaylist),
                        prev_playlist.clone(),
                    );

                    self.playbin.set_property("uri", Some(track_url.url));

                    self.play();
                }
            }
        }
    }
    pub fn skip_backward(&self, num: Option<usize>) {
        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            let one_second = ClockTime::from_seconds(1);

            if current_position > one_second && num.is_none() {
                self.playbin
                    .seek_simple(SeekFlags::FLUSH, ClockTime::default())
                    .expect("failed to seek");
                return;
            }
        }

        let mut prev_playlist = self.playlist_previous.write();
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
            let skipped_tracks = prev_playlist
                .0
                .drain(diff + 1..)
                .rev()
                .collect::<VecDeque<PlaylistTrack>>();

            for track in skipped_tracks {
                playlist.push_front(track);
            }
        }

        if let Some(prev_track) = prev_playlist.pop_back() {
            debug!("skipping backward to previous track");

            let track_url = if let Some(track_url) = prev_track.clone().track_url {
                track_url
            } else {
                let url_request = &self.url_request.sender;
                let url_return = &self.url_return.receiver;

                url_request
                    .send(prev_track.clone())
                    .expect("failed to send url to request");

                url_return
                    .recv()
                    .expect("failed to get track url")
                    .track_url
                    .expect("track url missing")
            };

            self.ready();

            self.state
                .player
                .insert::<String, PlaylistTrack>(AppKey::Player(PlayerKey::NextUp), prev_track);

            self.state
                .player
                .insert::<String, Playlist>(AppKey::Player(PlayerKey::Playlist), playlist.clone());
            self.state.player.insert::<String, Playlist>(
                AppKey::Player(PlayerKey::PreviousPlaylist),
                prev_playlist.clone(),
            );

            self.playbin.set_property("uri", Some(track_url.url));

            self.play();
        }
    }
    pub fn skip_to(&self, track_number: usize) -> bool {
        if track_number <= self.playlist().read().len() {
            self.skip_forward(Some(track_number));
            true
        } else {
            self.skip_backward(Some(track_number));
            true
        }
    }
    async fn broadcast_loop(&self) {
        loop {
            if self.playbin.current_state() != GstState::VoidPending
                || self.playbin.current_state() != GstState::Null
            {
                let pos: Option<ClockTime> = self.playbin.query_position();
                let dur: Option<ClockTime> = self.playbin.query_duration();
                let state = self.state.clone();

                if !self.playlist().read().is_empty() {
                    state.player.insert::<String, Playlist>(
                        AppKey::Player(PlayerKey::Playlist),
                        self.playlist.read().clone(),
                    );
                }

                if !self.playlist_previous.read().is_empty() {
                    state.player.insert::<String, Playlist>(
                        AppKey::Player(PlayerKey::PreviousPlaylist),
                        self.prev_playlist().read().clone(),
                    );
                }

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

                match self.broadcast_sender.send(state) {
                    Ok(_) => (),
                    Err(error) => {
                        error!("{:?}", error);
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
            }
        }
    }
    pub async fn play_track(&self, track: Track, quality: AudioQuality, client: Client) {
        let playlist_track = PlaylistTrack::new(track, Some(quality.clone()), None);
        self.playlist.write().push_back(playlist_track);
        self.start(quality, client).await;
    }
    pub async fn play_album(&self, album: Album, quality: AudioQuality, client: Client) {
        if let Some(tracklist) = album.to_playlist_tracklist(quality.clone()) {
            debug!("creating playlist");
            for playlist_track in tracklist {
                self.playlist.write().push_back(playlist_track);
            }

            self.start(quality, client).await;
        }
    }
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
    pub async fn setup(&self, client: Client) {
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
        tokio::spawn(async move {
            cloned_self.broadcast_loop().await;
        });

        let mut cloned_self = self.clone();
        tokio::spawn(async move {
            cloned_self.player_loop(client).await;
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
    async fn player_loop(&mut self, mut client: Client) {
        let mut messages = self.playbin.bus().unwrap().stream();
        let mut url_request = self.url_request.receiver.stream();

        loop {
            select! {
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

                            return;
                        },
                        MessageView::StreamStart(_) => {
                            let state = &mut self.state;

                            if let Some(next_track) = state.player.get::<String, PlaylistTrack>(AppKey::Player(PlayerKey::NextUp)) {
                               state.player.insert::<String, ClockValue>(AppKey::Player(PlayerKey::Duration),ClockTime::from_seconds(next_track.track.duration.try_into().unwrap()).into());
                            }
                        }
                        MessageView::Buffering(_) => {
                            // let state = &mut self.state;
                            //
                            // if !state.player.get_is_buffering() {
                            //     state.player.set_is_buffering(true);
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
