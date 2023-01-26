use crate::state::{ActiveScreen, ClockValue, FloatValue, StatusValue, TrackListValue};
use crate::ui::components::{Row, TableRows};
use qobuz_client::client::album::Album;
use qobuz_client::client::playlist::Playlist;
use qobuz_client::client::track::{TrackListTrack, TrackStatus};
use snafu::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender};
use tokio::sync::Mutex;

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

    pub fn set_duration(&mut self, remaining: ClockValue) {
        self.duration = remaining;
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

    pub fn skip_track(
        &mut self,
        index: Option<usize>,
        direction: SkipDirection,
    ) -> Option<TrackListTrack> {
        let next_track_index = if let Some(i) = index {
            i
        } else if let Some(current_track_index) = self.current_track_index() {
            if direction == SkipDirection::Forward {
                current_track_index + 1
            } else {
                current_track_index - 1
            }
        } else {
            0
        };

        self.tracklist.queue = self
            .tracklist
            .queue
            .iter_mut()
            .map(|mut t| {
                match t.index.cmp(&next_track_index) {
                    std::cmp::Ordering::Less => {
                        t.status = TrackStatus::Played;
                    }
                    std::cmp::Ordering::Equal => {
                        t.status = TrackStatus::Playing;
                    }
                    std::cmp::Ordering::Greater => {
                        t.status = TrackStatus::Unplayed;
                    }
                }

                t.to_owned()
            })
            .collect::<VecDeque<TrackListTrack>>();

        self.tracklist.find_track_by_index(next_track_index)
    }

    // pub fn next_track(&mut self, num: Option<usize>) -> Option<TrackListTrack> {
    //     // need previous track
    //     // if no num, get current track, get track index, increase by 1
    //     // if num, get track by num
    //     if let Some(next_track) = if let Some(current) = &self.current_track {
    //         self.tracklist
    //             .set_track_status(current.track.id as usize, TrackStatus::Played);

    //         if let Some(n) = num {
    //             self.tracklist.find_track_by_index(n)
    //         } else {
    //             self.tracklist.find_track_by_index(current.index + 1)
    //         }
    //     } else {
    //         self.tracklist.queue.front().cloned()
    //     } {
    //         self.tracklist
    //             .set_track_status(next_track.track.id as usize, TrackStatus::Playing);

    //         self.current_track = Some(next_track.clone());
    //         Some(next_track)
    //     } else {
    //         None
    //     }
    // }

    pub fn quitter(&self) -> BroadcastReceiver<bool> {
        self.quit_sender.subscribe()
    }

    pub fn quit(&self) {
        self.quit_sender
            .send(true)
            .expect("failed to send quit message");
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
}

impl Default for PlayerState {
    fn default() -> Self {
        let tracklist = TrackListValue::new(None);
        let (quit_sender, _) = tokio::sync::broadcast::channel::<bool>(1);

        Self {
            current_track: None,
            tracklist,
            duration_remaining: ClockValue::default(),
            duration: ClockValue::default(),
            position: ClockValue::default(),
            status: StatusValue(gstreamer::State::Null),
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
