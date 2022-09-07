use snafu::prelude::*;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

pub mod qobuz;
pub mod spotify;

#[derive(Hash, Clone, Eq, PartialEq)]
pub struct Isrc(String);

#[derive(Snafu, Debug)]
pub enum Error {
    ClientError { error: String },
}

impl From<spotify::Error> for Error {
    fn from(e: spotify::Error) -> Self {
        Error::ClientError {
            error: e.to_string(),
        }
    }
}

impl From<qobuz::Error> for Error {
    fn from(e: qobuz::Error) -> Self {
        Error::ClientError {
            error: e.to_string(),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
