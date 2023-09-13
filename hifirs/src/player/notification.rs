use gstreamer::{ClockTime, State};
use serde::{Deserialize, Serialize};

use crate::{player, player::queue::TrackListValue};

pub type BroadcastReceiver = async_broadcast::Receiver<Notification>;
pub type BroadcastSender = async_broadcast::Sender<Notification>;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Notification {
    Buffering {
        is_buffering: bool,
        percent: u32,
        target_status: State,
    },
    Status {
        status: State,
    },
    Position {
        clock: ClockTime,
    },
    CurrentTrackList {
        list: TrackListValue,
    },
    AudioQuality {
        bitdepth: u32,
        sampling_rate: u32,
    },
    Quit,
    Loading {
        is_loading: bool,
    },
    Error {
        error: player::error::Error,
    },
}
