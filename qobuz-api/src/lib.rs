use serde::{Deserialize, Serialize};
use snafu::prelude::*;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

pub mod client;

pub const TEST_TEMP_PATH: &str = "/tmp/hifirs_test";

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("No username provided."))]
    NoPassword,
    #[snafu(display("No password provided."))]
    NoUsername,
    #[snafu(display("No username or password provided."))]
    NoCredentials,
    #[snafu(display("No audio quality provided."))]
    NoQuality,
    #[snafu(display("Failed to get a usable secret from Qobuz."))]
    ActiveSecret,
    #[snafu(display("Failed to get an app id from Qobuz."))]
    AppID,
    #[snafu(display("Failed to login."))]
    Login,
    #[snafu(display("Authorization missing."))]
    Authorization,
    #[snafu(display("Failed to create client"))]
    Create,
    #[snafu(display("{message}"))]
    Api { message: String },
    #[snafu(display("Failed to deserialize json: {message}"))]
    DeserializeJSON { message: String },
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        let status = error.status();

        match status {
            Some(status) => Error::Api {
                message: status.to_string(),
            },
            None => Error::Api {
                message: "Error calling the API".to_string(),
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub username: Option<String>,
    pub password: Option<String>,
}
