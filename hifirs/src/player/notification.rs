use hifirs_qobuz_api::client::search_results::SearchAllResults;
use serde::{Deserialize, Serialize};

use crate::{
    player,
    qobuz::track::Track,
    state::{ClockValue, StatusValue, TrackListValue},
};

pub type BroadcastReceiver = async_broadcast::Receiver<Notification>;
pub type BroadcastSender = async_broadcast::Sender<Notification>;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
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
        clock: ClockValue,
    },
    Duration {
        clock: ClockValue,
    },
    CurrentTrackList {
        list: TrackListValue,
    },
    CurrentTrack {
        track: Track,
    },
    SearchResults {
        results: SearchAllResults,
    },
    Error {
        error: player::error::Error,
    },
}
