use crate::{action, action_blocking};
use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Play,
    Pause,
    PlayPause,
    Next,
    Previous,
    Stop,
    Quit,
    SkipTo { num: usize },
    SkipToById { track_id: usize },
    JumpForward,
    JumpBackward,
    PlayAlbum { album_id: String },
    PlayTrack { track_id: i32 },
    PlayUri { uri: String },
    PlayPlaylist { playlist_id: i64 },
}

/// Provides controls for other modules to send commands
/// to the player
#[derive(Debug, Clone)]
pub struct Controls {
    action_tx: Sender<Action>,
    action_rx: Receiver<Action>,
}

impl Controls {
    pub fn new() -> Controls {
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
    pub fn quit_blocking(&self) {
        action_blocking!(self, Action::Quit)
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

impl Default for Controls {
    fn default() -> Self {
        Self::new()
    }
}
