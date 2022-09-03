use serde::{Deserialize, Serialize};

pub mod album;
pub mod artist;
pub mod playlist;
pub mod search_results;
pub mod track;

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
    Playlist { id: String },
}

pub fn parse_url(string_url: &str) -> Option<UrlType> {
    if let Ok(url) = url::Url::parse(string_url) {
        if let (Some(host), Some(mut path)) = (url.host_str(), url.path_segments()) {
            if host == "play.qobuz.com" {
                debug!("got a qobuz url");

                match path.next() {
                    Some("album") => {
                        debug!("this is an album");
                        let id = path.next().unwrap().to_string();

                        Some(UrlType::Album { id })
                    }
                    Some("playlist") => {
                        debug!("this is a playlist");
                        let id = path.next().unwrap().to_string();

                        Some(UrlType::Playlist { id })
                    }
                    None => {
                        debug!("no path, cannot use path");
                        None
                    }
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}
