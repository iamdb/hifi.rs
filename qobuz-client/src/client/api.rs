use clap::ValueEnum;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Method, Response, StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snafu::prelude::*;
use std::collections::HashMap;

const BUNDLE_REGEX: &str =
    r#"<script src="(/resources/\d+\.\d+\.\d+-[a-z]\d{3}/bundle\.js)"></script>"#;
const APP_REGEX: &str = r#"cluster:"eu"}\):\(n.qobuzapi=\{app_id:"(?P<app_id>\d{9})",app_secret:"\w{32}",base_port:"80",base_url:"https://www\.qobuz\.com",base_method:"/api\.json/0\.2/"},n"#;
const SEED_REGEX: &str =
    r#"[a-z]\.initialSeed\("(?P<seed>[\w=]+)",window\.utimezone\.(?P<timezone>[a-z]+)\)"#;

macro_rules! format_info {
    () => {
        r#"name:"\w+/(?P<timezone>{}([a-z]?))",info:"(?P<info>[\w=]+)",extras:"(?P<extras>[\w=]+)""#
    };
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub username: Option<String>,
    pub password: Option<String>,
}

impl From<Vec<u8>> for Credentials {
    fn from(bytes: Vec<u8>) -> Self {
        let deserialized: Credentials =
            bincode::deserialize(&bytes).expect("failed to deserialize status value");

        deserialized
    }
}

impl From<Credentials> for Vec<u8> {
    fn from(creds: Credentials) -> Self {
        bincode::serialize(&creds).expect("failed to serialize string value")
    }
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("No username provided."))]
    NoPassword,
    #[snafu(display("No password provided."))]
    NoUsername,
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
            Some(StatusCode::BAD_REQUEST) => Error::Api {
                message: "Bad request".to_string(),
            },
            Some(StatusCode::UNAUTHORIZED) => Error::Api {
                message: "Unauthorized request".to_string(),
            },
            Some(StatusCode::NOT_FOUND) => Error::Api {
                message: "Item not found".to_string(),
            },
            Some(_) | None => Error::Api {
                message: "Error calling the API.".to_string(),
            },
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone)]
pub struct Client {
    secrets: HashMap<String, String>,
    active_secret: Option<String>,
    app_id: Option<String>,
    credentials: Option<Credentials>,
    base_url: String,
    client: reqwest::Client,
    default_quality: AudioQuality,
    user_token: Option<String>,
    bundle_regex: regex::Regex,
    app_id_regex: regex::Regex,
    seed_regex: regex::Regex,
}

pub async fn new(
    credentials: Option<Credentials>,
    active_secret: Option<String>,
    app_id: Option<String>,
    audio_quality: Option<AudioQuality>,
    user_token: Option<String>,
) -> Result<Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
            "User-Agent",
            HeaderValue::from_str(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36",
            )
            .unwrap(),
        );

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .default_headers(headers)
        .build()
        .unwrap();

    let default_quality = if let Some(quality) = audio_quality {
        quality
    } else {
        AudioQuality::Mp3
    };

    Ok(Client {
        client,
        secrets: HashMap::new(),
        active_secret,
        user_token,
        credentials,
        app_id,
        default_quality,
        base_url: "https://www.qobuz.com/api.json/0.2/".to_string(),
        bundle_regex: regex::Regex::new(BUNDLE_REGEX).unwrap(),
        app_id_regex: regex::Regex::new(APP_REGEX).unwrap(),
        seed_regex: regex::Regex::new(SEED_REGEX).unwrap(),
    })
}

#[non_exhaustive]
enum Endpoint {
    Album,
    Artist,
    Login,
    Track,
    UserPlaylist,
    SearchArtists,
    SearchAlbums,
    TrackURL,
    Playlist,
    Search,
}

impl Endpoint {
    fn as_str(&self) -> &'static str {
        match self {
            Endpoint::Album => "album/get",
            Endpoint::Artist => "artist/get",
            Endpoint::Login => "user/login",
            Endpoint::Track => "track/get",
            Endpoint::SearchArtists => "artist/search",
            Endpoint::UserPlaylist => "playlist/getUserPlaylists",
            Endpoint::SearchAlbums => "album/search",
            Endpoint::Search => "catalog/search",
            Endpoint::TrackURL => "track/getFileUrl",
            Endpoint::Playlist => "playlist/get",
        }
    }
}

macro_rules! call {
    ($self:ident, $endpoint:expr, $params:expr) => {
        match $self.make_call($endpoint, $params).await {
            Ok(response) => match serde_json::from_str(response.as_str()) {
                Ok(item) => Ok(item),
                Err(error) => Err(Error::DeserializeJSON {
                    message: error.to_string(),
                }),
            },
            Err(error) => Err(Error::Api {
                message: error.to_string(),
            }),
        }
    };
}

#[allow(unused)]
impl Client {
    pub fn quality(&self) -> AudioQuality {
        self.default_quality.clone()
    }

    /// Setup app_id, secret and user credentials for authentication
    pub async fn setup(&mut self) -> Result<Self> {
        info!("setting up the api client");

        let mut refresh_config = false;

        if self.app_id.is_none() || self.active_secret.is_none() {
            refresh_config = true;
        }

        if refresh_config {
            self.get_config().await.expect("failed to get config");
            self.test_secrets().await.expect("failed to get secrets");
        }

        if self.credentials.is_none() {
            error!("credentials missing");
        }

        Ok(self.clone())
    }

    pub async fn refresh(&mut self) {
        self.get_config().await.expect("failed to get config");
        self.test_secrets().await.expect("failed to get secrets");
    }

    /// Login a user
    pub async fn login(&mut self) -> Result<()> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::Login.as_str());
        let app_id = self.app_id.clone().unwrap();
        let username = self
            .credentials
            .as_ref()
            .unwrap()
            .username
            .clone()
            .expect("tried to login without username.");
        let password = self
            .credentials
            .as_ref()
            .unwrap()
            .password
            .clone()
            .expect("tried to login without password.");

        info!(
            "logging in with email ({}) and password **HIDDEN** for app_id {}",
            username, app_id
        );

        let params = vec![
            ("email", username.as_str()),
            ("password", password.as_str()),
            ("app_id", app_id.as_str()),
        ];

        match self.make_call(endpoint, Some(params)).await {
            Ok(response) => {
                let json: Value = serde_json::from_str(response.as_str()).unwrap();
                info!("Successfully logged in");
                debug!("{}", json);
                let mut token = json["user_auth_token"].to_string();
                token = token[1..token.len() - 1].to_string();

                self.user_token = Some(token);
                Ok(())
            }
            Err(_) => Err(Error::Login),
        }
    }

    /// Retrieve a list of the user's playlists
    pub async fn user_playlists(&self) -> Result<UserPlaylistsResult> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::UserPlaylist.as_str());
        let params = vec![("limit", "500"), ("extra", "tracks"), ("offset", "0")];

        call!(self, endpoint, Some(params))
    }

    /// Retrieve a playlist
    pub async fn playlist(&self, playlist_id: String) -> Result<Playlist> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::Playlist.as_str());
        let mut params = vec![
            ("limit", "500"),
            ("extra", "tracks"),
            ("playlist_id", playlist_id.as_str()),
            ("offset", "0"),
        ];
        let mut fetched_tracks = 0;

        let playlist: Result<Playlist> = call!(self, endpoint.clone(), Some(params.clone()));

        if let Ok(playlist) = playlist {
            if let Ok(all_items_playlist) = self.playlist_items(&playlist, endpoint, params).await {
                Ok(all_items_playlist.clone())
            } else {
                Err(Error::Api {
                    message: "error fetching playlist".to_string(),
                })
            }
        } else {
            Err(Error::Api {
                message: "error fetching playlist".to_string(),
            })
        }
    }

    async fn playlist_items<'p>(
        &self,
        mut playlist: &'p Playlist,
        endpoint: String,
        params: Vec<(&str, &str)>,
    ) -> Result<&'p Playlist> {
        let mut params = params.clone();
        let total_tracks = playlist.tracks_count as usize;
        let mut all_tracks: Vec<Track> = Vec::new();

        if let Some(mut tracks) = playlist.tracks.clone() {
            all_tracks.append(&mut tracks.items);

            while all_tracks.len() < total_tracks {
                let id = playlist.id.to_string();
                let limit_string = (total_tracks - all_tracks.len()).to_string();
                let offset_string = all_tracks.len().to_string();

                let mut params = vec![
                    ("limit", limit_string.as_str()),
                    ("extra", "tracks"),
                    ("playlist_id", id.as_str()),
                    ("offset", offset_string.as_str()),
                ];

                let playlist: Result<Playlist> =
                    call!(self, endpoint.clone(), Some(params.clone()));

                match &playlist {
                    Ok(playlist) => {
                        debug!("appending tracks to playlist");
                        if let Some(new_tracks) = &playlist.tracks {
                            all_tracks.append(&mut new_tracks.clone().items);
                        }
                    }
                    Err(error) => error!("{}", error.to_string()),
                }
            }

            if !all_tracks.is_empty() {
                tracks.items = all_tracks;
            }
        }

        Ok(playlist)
    }

    /// Retrieve track information
    pub async fn track(&self, track_id: i32) -> Result<Track> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::Track.as_str());
        let track_id_string = track_id.to_string();
        let params = vec![("track_id", track_id_string.as_str())];

        call!(self, endpoint, Some(params))
    }

    /// Retrieve url information for a track's audio file
    pub async fn track_url(
        &self,
        track_id: i32,
        fmt_id: Option<AudioQuality>,
        sec: Option<String>,
    ) -> Result<TrackURL> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::TrackURL.as_str());
        let now = format!("{}", chrono::Utc::now().timestamp());
        let secret = if let Some(secret) = sec {
            secret
        } else if let Some(secret) = &self.active_secret {
            secret.clone()
        } else {
            return Err(Error::ActiveSecret);
        };

        let format_id = if let Some(quality) = fmt_id {
            quality
        } else {
            self.quality()
        };

        let sig = format!(
            "trackgetFileUrlformat_id{}intentstreamtrack_id{}{}{}",
            format_id.clone(),
            track_id,
            now,
            secret
        );
        let hashed_sig = format!("{:x}", md5::compute(sig.as_str()));

        let track_id = track_id.to_string();
        let format_string = format_id.to_string();

        let params = vec![
            ("request_ts", now.as_str()),
            ("request_sig", hashed_sig.as_str()),
            ("track_id", track_id.as_str()),
            ("format_id", format_string.as_str()),
            ("intent", "stream"),
        ];

        call!(self, endpoint, Some(params))
    }

    pub async fn search_all(&self, query: String) -> Result<String> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::Search.as_str());
        let params = vec![("query", query.as_str()), ("limit", "500")];

        call!(self, endpoint, Some(params))
    }

    // Retrieve information about an album
    pub async fn album(&self, album_id: String) -> Result<Album> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::Album.as_str());
        let params = vec![("album_id", album_id.as_str())];

        call!(self, endpoint, Some(params))
    }

    // Search the database for albums
    pub async fn search_albums(
        &self,
        query: String,
        limit: Option<i32>,
    ) -> Result<AlbumSearchResults> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::SearchAlbums.as_str());
        let limit = if let Some(limit) = limit {
            limit.to_string()
        } else {
            100.to_string()
        };
        let params = vec![("query", query.as_str()), ("limit", limit.as_str())];

        call!(self, endpoint, Some(params))
    }

    // Retrieve information about an artist
    pub async fn artist(&self, artist_id: i32, limit: Option<i32>) -> Result<Artist> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::Artist.as_str());
        let app_id = self.app_id.clone();
        let limit = if let Some(limit) = limit {
            limit.to_string()
        } else {
            100.to_string()
        };

        let artistid_string = artist_id.to_string();

        let params = vec![
            ("artist_id", artistid_string.as_str()),
            (
                "app_id",
                app_id
                    .as_ref()
                    .expect("missing app id. this should not have happened.")
                    .as_str(),
            ),
            ("limit", limit.as_str()),
            ("offset", "0"),
            ("extra", "albums"),
        ];

        call!(self, endpoint, Some(params))
    }

    // Search the database for artists
    pub async fn search_artists(
        &self,
        query: String,
        limit: Option<i32>,
    ) -> Result<ArtistSearchResults> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::SearchArtists.as_str());
        let limit = if let Some(limit) = limit {
            limit.to_string()
        } else {
            100.to_string()
        };
        let params = vec![("query", query.as_str()), ("limit", &limit)];

        call!(self, endpoint, Some(params))
    }

    // Set a user access token for authentication
    pub fn set_token(&mut self, token: String) {
        self.user_token = Some(token);
    }

    // Set a username for authentication
    pub fn set_credentials(&mut self, credentials: Credentials) {
        self.credentials = Some(credentials);
    }

    // Set an app_id for authentication
    pub fn set_app_id(&mut self, app_id: String) {
        self.app_id = Some(app_id);
    }

    // Set an app secret for authentication
    pub fn set_active_secret(&mut self, active_secret: String) {
        self.active_secret = Some(active_secret);
    }

    pub fn set_default_quality(&mut self, quality: AudioQuality) {
        self.default_quality = quality;
    }

    // Call the api and retrieve the JSON payload
    async fn make_call(
        &self,
        endpoint: String,
        params: Option<Vec<(&str, &str)>>,
    ) -> Result<String> {
        let mut headers = HeaderMap::new();

        if let Some(app_id) = &self.app_id {
            info!("adding app_id to request headers: {}", app_id);
            headers.insert("X-App-Id", HeaderValue::from_str(app_id.as_str()).unwrap());
        } else {
            error!("no app_id");
        }

        if let Some(token) = &self.user_token {
            info!("adding token to request headers: {}", token);
            headers.insert(
                "X-User-Auth-Token",
                HeaderValue::from_str(token.as_str()).unwrap(),
            );
        }

        debug!("calling {} endpoint", endpoint);
        let request = self.client.request(Method::GET, endpoint).headers(headers);

        if let Some(p) = params {
            let response = request.query(&p).send().await?;
            self.handle_response(response).await
        } else {
            let response = request.send().await?;
            self.handle_response(response).await
        }
    }

    // Handle a response retrieved from the api
    async fn handle_response(&self, response: Response) -> Result<String> {
        match response.status() {
            StatusCode::BAD_REQUEST => Err(Error::Api {
                message: "Bad request".to_string(),
            }),
            StatusCode::UNAUTHORIZED => Err(Error::Api {
                message: "Unauthorized request".to_string(),
            }),
            StatusCode::NOT_FOUND => Err(Error::Api {
                message: "Item not found".to_string(),
            }),
            StatusCode::OK => {
                let res = response.text().await.unwrap();
                Ok(res)
            }
            _ => unreachable!(),
        }
    }

    // ported from https://github.com/vitiko98/qobuz-dl/blob/master/qobuz_dl/bundle.py
    // Retrieve the app_id and generate the secrets needed to authenticate
    async fn get_config(&mut self) -> Result<()> {
        let play_url = "https://play.qobuz.com";
        let login_page = self
            .client
            .get(format!("{}/login", play_url))
            .send()
            .await
            .expect("failed to get login page. something is very wrong.");

        let contents = login_page.text().await.unwrap();

        let bundle_path = self
            .bundle_regex
            .captures(contents.as_str())
            .expect("regex failed")
            .get(1)
            .map_or("", |m| m.as_str());

        let bundle_url = format!("{}{}", play_url, bundle_path);
        let bundle_page = self.client.get(bundle_url).send().await.unwrap();

        let bundle_contents = bundle_page.text().await.unwrap();

        if let Some(captures) = self.app_id_regex.captures(bundle_contents.as_str()) {
            let app_id = captures
                .name("app_id")
                .map_or("".to_string(), |m| m.as_str().to_string());

            self.app_id = Some(app_id.clone());

            let seed_data = self.seed_regex.captures_iter(bundle_contents.as_str());

            seed_data.for_each(|s| {
                let seed = s.name("seed").map_or("", |m| m.as_str()).to_string();
                let timezone = s.name("timezone").map_or("", |m| m.as_str()).to_string();

                let info_regex = format!(format_info!(), util::capitalize(&timezone));
                let info_regex_str = info_regex.as_str();
                regex::Regex::new(info_regex_str)
                    .unwrap()
                    .captures_iter(bundle_contents.as_str())
                    .for_each(|c| {
                        let timezone = c.name("timezone").map_or("", |m| m.as_str()).to_string();
                        let info = c.name("info").map_or("", |m| m.as_str()).to_string();
                        let extras = c.name("extras").map_or("", |m| m.as_str()).to_string();

                        let chars = format!("{}{}{}", seed, info, extras);
                        let encoded_secret = chars[..chars.len() - 44].to_string();
                        let decoded_secret =
                            base64::decode(encoded_secret).expect("failed to decode base64 secret");
                        let secret_utf8 = std::str::from_utf8(&decoded_secret)
                            .expect("failed to convert base64 to string")
                            .to_string();

                        debug!("{}\t{}\t{}", app_id, timezone.to_lowercase(), secret_utf8);
                        self.secrets.insert(timezone, secret_utf8);
                    });
            });

            Ok(())
        } else {
            Err(Error::AppID)
        }
    }

    // Check the retrieved secrets to see which one works.
    async fn test_secrets(&mut self) -> Result<()> {
        debug!("testing secrets");
        let secrets = self.secrets.clone();
        let mut active_secret: Option<String> = None;

        for (timezone, secret) in secrets.iter() {
            let response = self
                .track_url(5966783, Some(AudioQuality::Mp3), Some(secret.to_string()))
                .await;

            if response.is_ok() {
                debug!("found good secret: {}\t{}", timezone, secret);
                let secret_string = secret.to_string();
                active_secret = Some(secret_string);

                break;
            };
        }

        if let Some(secret) = active_secret {
            self.set_active_secret(secret);
            Ok(())
        } else {
            Err(Error::ActiveSecret)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ValueEnum)]
pub enum OutputFormat {
    Json,
    Tsv,
}

use crate::client::artist::{Artist, ArtistSearchResults};
use crate::client::playlist::{Playlist, UserPlaylistsResult};
use crate::client::track::Track;
use crate::client::TrackURL;
use crate::client::{
    album::{Album, AlbumSearchResults},
    AudioQuality,
};

#[tokio::test]
async fn can_use_methods() {
    use tokio_test::assert_ok;

    let creds = Credentials {
        username: Some(env!("QOBUZ_USERNAME").to_string()),
        password: Some(env!("QOBUZ_PASSWORD").to_string()),
    };

    let mut client = new(Some(creds.clone()), None, None, None, None)
        .await
        .expect("failed to create client");

    client.refresh().await;
    client.login().await.expect("failed to login");

    assert_ok!(client.user_playlists().await);
    let album_response = assert_ok!(
        client
            .search_albums("a love supreme".to_string(), Some(10))
            .await
    );
    assert_eq!(album_response.albums.items.len(), 10);
    assert_ok!(client.album("lhrak0dpdxcbc".to_string()).await);
    let artist_response = assert_ok!(
        client
            .search_artists("pink floyd".to_string(), Some(10))
            .await
    );
    assert_eq!(artist_response.artists.items.len(), 10);
    assert_ok!(client.artist(148745, Some(10)).await);
    assert_ok!(client.track(155999429).await);
    assert_ok!(
        client
            .track_url(155999429, Some(AudioQuality::Mp3), None)
            .await
    );
}
