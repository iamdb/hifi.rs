use gstreamer::{ClockTime, State};
use serde::{Deserialize, Serialize, Serializer};

use crate::{player, player::queue::TrackListValue};

pub type BroadcastReceiver = async_broadcast::Receiver<Notification>;
pub type BroadcastSender = async_broadcast::Sender<Notification>;

fn serialize_clocktime<S>(clock: &ClockTime, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    clock.seconds().serialize(s)
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Notification {
    Buffering {
        is_buffering: bool,
        percent: u32,
        target_state: State,
    },
    Status {
        status: State,
    },
    Position {
        #[serde(serialize_with = "serialize_clocktime")]
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
        target_state: State,
    },
    Error {
        error: player::error::Error,
    },
}
