use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use snafu::prelude::*;
use std::fmt::Display;

pub mod album;
pub mod api;
pub mod artist;
pub mod playlist;
pub mod search_results;
pub mod track;

#[derive(Default, Debug)]
pub struct ApiConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    pub default_quality: Option<i64>,
    pub user_token: Option<String>,
    pub app_id: Option<String>,
    pub active_secret: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Composer {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub albums_count: i64,
    pub image: Option<Image>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub small: String,
    pub thumbnail: Option<String>,
    pub large: String,
    pub back: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackURL {
    pub track_id: i32,
    pub duration: i32,
    pub url: String,
    pub format_id: i32,
    pub mime_type: String,
    pub sampling_rate: f64,
    pub bit_depth: i32,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub login: String,
}

pub enum UrlType {
    Album { id: String },
    Playlist { id: i64 },
    Track { id: i32 },
}

#[derive(Snafu, Debug)]
pub enum UrlTypeError {
    #[snafu(display("This uri contains an unfamiliar domain."))]
    WrongDomain,
    #[snafu(display("the url contains an invalid path"))]
    InvalidPath,
    #[snafu(display("the url is invalid."))]
    InvalidUrl,
    #[snafu(display("an unknown error has occurred"))]
    Unknown,
}

pub type ParseUrlResult<T, E = UrlTypeError> = std::result::Result<T, E>;

/// The audio quality as defined by the Qobuz API.
#[derive(Default, Clone, Debug, Serialize, Deserialize, ValueEnum)]
pub enum AudioQuality {
    #[default]
    Mp3 = 5,
    CD = 6,
    HIFI96 = 7,
    HIFI192 = 27,
    Unknown,
}

impl From<i64> for AudioQuality {
    fn from(quality_id: i64) -> Self {
        match quality_id {
            5 => Self::Mp3,
            6 => Self::CD,
            7 => Self::HIFI96,
            27 => Self::HIFI192,
            _ => Self::Unknown,
        }
    }
}

impl Display for AudioQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.clone() as u32))
    }
}

pub fn parse_url(string_url: &str) -> ParseUrlResult<UrlType> {
    if let Ok(url) = url::Url::parse(string_url) {
        if let (Some(host), Some(mut path)) = (url.host_str(), url.path_segments()) {
            if host == "play.qobuz.com" || host == "open.qobuz.com" {
                debug!("got a qobuz url");

                match path.next() {
                    Some("album") => {
                        debug!("this is an album");
                        let id = path.next().unwrap().to_string();

                        Ok(UrlType::Album { id })
                    }
                    Some("playlist") => {
                        debug!("this is a playlist");
                        let id = path
                            .next()
                            .unwrap()
                            .parse::<i64>()
                            .expect("failed to convert id");

                        Ok(UrlType::Playlist { id })
                    }
                    Some("track") => {
                        debug!("this is a track");
                        let id = path
                            .next()
                            .unwrap()
                            .parse::<i32>()
                            .expect("failed to convert id");

                        Ok(UrlType::Track { id })
                    }
                    None => {
                        debug!("no path, cannot use path");
                        Err(UrlTypeError::InvalidPath)
                    }
                    _ => Err(UrlTypeError::Unknown),
                }
            } else {
                Err(UrlTypeError::WrongDomain)
            }
        } else {
            Err(UrlTypeError::InvalidUrl)
        }
    } else {
        Err(UrlTypeError::InvalidUrl)
    }
}

pub fn capitalize(s: &mut str) {
    if let Some(r) = s.get_mut(0..1) {
        r.make_ascii_uppercase();
    }
}
