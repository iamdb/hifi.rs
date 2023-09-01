use gstreamer::{glib, traits::GstObjectExt, StateChangeError};
use serde::{Deserialize, Serialize};
use snafu::prelude::*;

use crate::player::notification::Notification;

#[derive(Snafu, Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    #[snafu(display("{message}"))]
    FailedToPlay {
        message: String,
    },
    #[snafu(display("failed to retrieve a track url"))]
    TrackURL,
    #[snafu(display("failed to seek"))]
    Seek,
    #[snafu(display("sorry, could not resume previous session"))]
    Resume,
    #[snafu(display("{message}"))]
    GStreamer {
        message: String,
    },
    #[snafu(display("{message}"))]
    Client {
        message: String,
    },
    NotificationError,
    App,
}

impl From<glib::Error> for Error {
    fn from(value: glib::Error) -> Self {
        Error::GStreamer {
            message: value.to_string(),
        }
    }
}

impl From<glib::BoolError> for Error {
    fn from(value: glib::BoolError) -> Self {
        Error::GStreamer {
            message: value.to_string(),
        }
    }
}

impl From<StateChangeError> for Error {
    fn from(value: StateChangeError) -> Self {
        Error::GStreamer {
            message: value.to_string(),
        }
    }
}

impl From<hifirs_qobuz_api::Error> for Error {
    fn from(value: hifirs_qobuz_api::Error) -> Self {
        Error::Client {
            message: value.to_string(),
        }
    }
}

impl From<flume::SendError<Notification>> for Error {
    fn from(_value: flume::SendError<Notification>) -> Self {
        Self::NotificationError
    }
}

impl From<async_broadcast::SendError<Notification>> for Error {
    fn from(_value: async_broadcast::SendError<Notification>) -> Self {
        Self::NotificationError
    }
}

impl From<&gstreamer::message::Error> for Error {
    fn from(value: &gstreamer::message::Error) -> Self {
        let error = format!(
            "Error from {:?}: {} ({:?})",
            value.src().map(|s| s.path_string()),
            value.error(),
            value.debug()
        );
        Error::GStreamer { message: error }
    }
}
