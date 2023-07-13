use hifirs_qobuz_api::client::track::TrackListTrack;

use crate::{
    player,
    state::{ClockValue, StatusValue, TrackListValue},
};

pub type BroadcastReceiver = async_broadcast::Receiver<Notification>;
pub type BroadcastSender = async_broadcast::Sender<Notification>;

#[derive(Debug, Clone)]
pub enum Notification {
    Buffering {
        is_buffering: bool,
        percent: i32,
        target_status: StatusValue,
    },
    Status {
        status: StatusValue,
    },
    Position {
        position: ClockValue,
    },
    Duration {
        duration: ClockValue,
    },
    CurrentTrackList {
        list: TrackListValue,
    },
    CurrentTrack {
        track: TrackListTrack,
    },
    Error {
        error: player::error::Error,
    },
}
