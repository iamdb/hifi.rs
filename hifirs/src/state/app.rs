use crate::{
    state::{ActiveScreen, ClockValue, FloatValue, StatusValue, TrackListValue},
    ui::components::{Row, TableRows},
};
use qobuz_client::client::{
    album::Album,
    api::Client,
    playlist::Playlist,
    track::{TrackListTrack, TrackStatus},
};
use snafu::prelude::*;
use std::{fmt::Display, sync::Arc};
use tokio::sync::{
    broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender},
    Mutex,
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Collection not found."))]
    CollectionNotFound,
    #[snafu(display("Unsupported."))]
    Unsupported,
    #[snafu(display("Reportable bug."))]
    ReportableBug,
    #[snafu(display("Database in use."))]
    Io,
    #[snafu(display("Database corrupted."))]
    Corruption,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone)]
pub struct PlayerState {
    client: Client,
    current_track: Option<TrackListTrack>,
    tracklist: TrackListValue,
    current_progress: FloatValue,
    duration_remaining: ClockValue,
    duration: ClockValue,
    position: ClockValue,
    status: StatusValue,
    is_buffering: bool,
    resume: bool,
    active_screen: ActiveScreen,
    target_status: StatusValue,
    quit_sender: BroadcastSender<bool>,
}

pub type SafePlayerState = Arc<Mutex<PlayerState>>;

impl PlayerState {
    pub fn set_active_screen(&mut self, screen: ActiveScreen) {
        self.active_screen = screen;
    }

    pub fn active_screen(&self) -> ActiveScreen {
        self.active_screen.clone()
    }

    pub fn set_status(&mut self, status: StatusValue) {
        self.status = status;
    }

    pub fn status(&self) -> StatusValue {
        self.status.clone()
    }

    pub fn set_current_track(&mut self, track: TrackListTrack) {
        self.current_track = Some(track);
    }

    pub fn set_position(&mut self, position: ClockValue) {
        self.position = position;
    }

    pub fn position(&self) -> ClockValue {
        self.position.clone()
    }

    pub fn set_buffering(&mut self, buffering: bool) {
        self.is_buffering = buffering;
    }

    pub fn buffering(&self) -> bool {
        self.is_buffering
    }

    pub fn set_resume(&mut self, resume: bool) {
        self.resume = resume;
    }

    pub fn set_current_progress(&mut self, progress: FloatValue) {
        self.current_progress = progress;
    }

    pub fn progress(&self) -> FloatValue {
        self.current_progress.clone()
    }

    pub fn set_duration_remaining(&mut self, remaining: ClockValue) {
        self.duration_remaining = remaining;
    }

    pub fn set_duration(&mut self, duration: ClockValue) {
        self.duration = duration;
    }

    pub fn duration(&self) -> ClockValue {
        self.duration.clone()
    }

    pub fn current_track(&self) -> Option<TrackListTrack> {
        self.current_track.clone()
    }

    pub fn unplayed_tracks(&self) -> Vec<&TrackListTrack> {
        self.tracklist.unplayed_tracks()
    }

    pub fn played_tracks(&self) -> Vec<&TrackListTrack> {
        self.tracklist.played_tracks()
    }

    pub fn album(&self) -> Option<&Album> {
        self.tracklist.get_album()
    }

    pub fn playlist(&self) -> Option<&Playlist> {
        self.tracklist.get_playlist()
    }

    pub fn current_track_index(&self) -> Option<usize> {
        self.current_track.as_ref().map(|track| track.index)
    }

    pub fn replace_list(&mut self, tracklist: TrackListValue) {
        debug!("replacing tracklist");
        self.tracklist = tracklist;
    }

    pub fn track_index(&self, track_id: usize) -> Option<usize> {
        if let Some(track) = self.tracklist.find_track(track_id) {
            Some(track.index)
        } else {
            None
        }
    }

    pub fn set_track_status(&mut self, track_id: usize, status: TrackStatus) {
        self.tracklist.set_track_status(track_id, status);
    }

    pub fn rows(&self) -> Vec<Row> {
        self.tracklist.rows()
    }

    /// Attach a `TrackURL` to the given track.
    pub async fn attach_track_url(&mut self, track: &mut TrackListTrack) {
        if let Ok(track_url) = self.client.track_url(track.track.id, None, None).await {
            track.set_track_url(track_url);
        }
    }

    pub async fn attach_track_url_current(&mut self) {
        if let Some(current_track) = self.current_track.as_mut() {
            if let Ok(track_url) = self
                .client
                .track_url(current_track.track.id, None, None)
                .await
            {
                current_track.set_track_url(track_url);
            }
        }
    }

    pub async fn skip_track(
        &mut self,
        index: Option<usize>,
        direction: SkipDirection,
    ) -> Option<TrackListTrack> {
        let next_track_index = if let Some(i) = index {
            i
        } else if let Some(current_track_index) = self.current_track_index() {
            if direction == SkipDirection::Forward {
                if current_track_index < self.tracklist.len() {
                    current_track_index + 1
                } else {
                    self.tracklist.len()
                }
            } else if current_track_index > 1 {
                current_track_index - 1
            } else {
                0
            }
        } else {
            0
        };

        self.tracklist
            .queue
            .iter_mut()
            .for_each(|mut t| match t.index.cmp(&next_track_index) {
                std::cmp::Ordering::Less => {
                    t.status = TrackStatus::Played;
                }
                std::cmp::Ordering::Equal => {
                    t.status = TrackStatus::Playing;
                    self.current_track = Some(t.clone());
                }
                std::cmp::Ordering::Greater => {
                    t.status = TrackStatus::Unplayed;
                }
            });

        self.attach_track_url_current().await;
        self.current_track.clone()
    }

    pub fn reset_player(&mut self) {
        self.duration = ClockValue::default();
        self.position = ClockValue::default();
        self.current_progress = FloatValue(0.0);
    }

    pub fn quitter(&self) -> BroadcastReceiver<bool> {
        self.quit_sender.subscribe()
    }

    pub fn quit(&self) {
        self.quit_sender
            .send(true)
            .expect("failed to send quit message");
    }

    pub fn set_target_status(&mut self, status: StatusValue) {
        self.target_status = status;
    }

    pub fn target_status(&self) -> StatusValue {
        self.target_status.clone()
    }

    pub fn reset(&mut self) {
        self.tracklist.clear();
        self.current_track = None;
        self.current_progress = FloatValue(0.0);
        self.duration_remaining = ClockValue::default();
        self.duration = ClockValue::default();
        self.position = ClockValue::default();
        self.status = gstreamer::State::Null.into();
        self.is_buffering = false;
        self.resume = false;
    }
    pub fn new(client: Client) -> Self {
        let tracklist = TrackListValue::new(None);
        let (quit_sender, _) = tokio::sync::broadcast::channel::<bool>(1);

        Self {
            current_track: None,
            client,
            tracklist,
            duration_remaining: ClockValue::default(),
            duration: ClockValue::default(),
            position: ClockValue::default(),
            status: StatusValue(gstreamer::State::Null),
            target_status: StatusValue(gstreamer::State::Null),
            current_progress: FloatValue(0.0),
            is_buffering: false,
            resume: false,
            active_screen: ActiveScreen::NowPlaying,
            quit_sender,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipDirection {
    Forward,
    Backward,
}

impl Display for SkipDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkipDirection::Forward => f.write_str("forward"),
            SkipDirection::Backward => f.write_str("backward"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StateKey {
    App(AppKey),
    Client(ClientKey),
    Player(PlayerKey),
}

impl StateKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            StateKey::App(key) => key.as_str(),
            StateKey::Client(key) => key.as_str(),
            StateKey::Player(key) => key.as_str(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppKey {
    ActiveScreen,
}

impl AppKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppKey::ActiveScreen => "active_screen",
        }
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ClientKey {
    ActiveSecret,
    AppID,
    DefaultQuality,
    Password,
    Token,
    Username,
}

impl ClientKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClientKey::ActiveSecret => "active_secret",
            ClientKey::AppID => "app_id",
            ClientKey::DefaultQuality => "default_quality",
            ClientKey::Password => "password",
            ClientKey::Token => "token",
            ClientKey::Username => "username",
        }
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PlayerKey {
    Duration,
    DurationRemaining,
    NextUp,
    Playlist,
    Position,
    PreviousPlaylist,
    Progress,
    Status,
}

impl PlayerKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlayerKey::Duration => "duration",
            PlayerKey::DurationRemaining => "duration_remaining",
            PlayerKey::NextUp => "next_up",
            PlayerKey::Playlist => "playlist",
            PlayerKey::Position => "position",
            PlayerKey::PreviousPlaylist => "prev_playlist",
            PlayerKey::Progress => "progress",
            PlayerKey::Status => "status",
        }
    }
}
