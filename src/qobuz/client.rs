use super::{
    Album, AlbumSearchResults, Artist, ArtistSearchResults, Playlist, Track, TrackURL,
    UserPlaylists,
};
use crate::{
    get_client,
    state::{
        app::{AppState, ClientKey, StateKey},
        AudioQuality, StringValue,
    },
    Credentials,
};
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
    API { message: String },
    #[snafu(display("Failed to deserialize json: {message}"))]
    DeserializeJSON { message: String },
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        let status = error.status();

        match status {
            Some(StatusCode::BAD_REQUEST) => Error::API {
                message: "Bad request".to_string(),
            },
            Some(StatusCode::UNAUTHORIZED) => Error::API {
                message: "Unauthorized request".to_string(),
            },
            Some(StatusCode::NOT_FOUND) => Error::API {
                message: "Item not found".to_string(),
            },
            Some(_) | None => Error::API {
                message: "Error calling the API.".to_string(),
            },
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone)]
pub struct Client {
    secrets: HashMap<String, String>,
    active_secret: Option<StringValue>,
    app_id: Option<StringValue>,
    username: Option<StringValue>,
    password: Option<StringValue>,
    base_url: String,
    client: reqwest::Client,
    pub default_quality: AudioQuality,
    user_token: Option<StringValue>,
    bundle_regex: regex::Regex,
    app_id_regex: regex::Regex,
    seed_regex: regex::Regex,
    state: AppState,
}

pub async fn new(state: AppState, creds: Credentials) -> Result<Client> {
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

    let tree = state.config.clone();
    let default_quality =
        if let Some(quality) = get_client!(ClientKey::DefaultQuality, tree, AudioQuality) {
            quality
        } else {
            AudioQuality::Mp3
        };

    Client {
        client,
        secrets: HashMap::new(),
        active_secret: None,
        user_token: None,
        app_id: None,
        username: None,
        state,
        password: None,
        default_quality,
        base_url: "https://www.qobuz.com/api.json/0.2/".to_string(),
        bundle_regex: regex::Regex::new(BUNDLE_REGEX).unwrap(),
        app_id_regex: regex::Regex::new(APP_REGEX).unwrap(),
        seed_regex: regex::Regex::new(SEED_REGEX).unwrap(),
    }
    .setup(creds)
    .await
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
            Err(error) => Err(Error::API {
                message: error.to_string(),
            }),
        }
    };
}

impl Client {
    pub fn quality(&self) -> AudioQuality {
        self.default_quality.clone()
    }
    /// Setup app_id, secret and user credentials for authentication
    pub async fn setup(&mut self, creds: Credentials) -> Result<Self> {
        info!("setting up the api client");

        let mut refresh_config = false;
        let tree = self.state.config.clone();

        if let Some(app_id) = get_client!(ClientKey::AppID, tree, StringValue) {
            info!("using app_id from cache: {}", app_id);
            self.set_app_id(Some(app_id));
        } else {
            self.set_app_id(None);
            refresh_config = true;
        }

        if let Some(active_secret) = get_client!(ClientKey::ActiveSecret, tree, StringValue) {
            info!("using app_secret from cache: {}", active_secret);
            self.set_active_secret(Some(active_secret));
        } else {
            self.set_active_secret(None);
            self.set_app_id(None);
            refresh_config = true;
        }

        if refresh_config {
            self.get_config().await.expect("failed to get config");
            self.test_secrets().await.expect("failed to get secrets");
        }

        if let Some(token) = get_client!(ClientKey::Token, tree, StringValue) {
            info!("using token from cache");
            self.set_token(token);

            Ok(self.clone())
        } else {
            if let Some(u) = creds.username {
                debug!("using username from cli argument: {}", u);
                self.set_username(u.into());
            } else if let Some(u) = get_client!(ClientKey::Username, tree, StringValue) {
                debug!("using username stored in database: {}", u);
                self.set_username(u);
            } else {
                return Err(Error::NoUsername);
            }

            if let Some(p) = creds.password {
                debug!("using password from cli argument: {}", p);
                self.set_password(p.into());
            } else if let Some(p) = get_client!(ClientKey::Password, tree, StringValue) {
                debug!("using password stored in database: {}", p);
                self.set_password(p);
            } else {
                return Err(Error::NoPassword);
            }

            if self.username.is_some() && self.password.is_some() {
                if self.login().await.is_ok() {
                    Ok(self.clone())
                } else {
                    Err(Error::Login)
                }
            } else {
                Err(Error::Create)
            }
        }
    }

    /// Login a user
    pub async fn login(&mut self) -> Result<()> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::Login.as_str());
        let app_id = self.app_id.clone().unwrap();
        let username = self
            .username
            .clone()
            .expect("tried to login without username.");
        let password = self
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

                self.user_token = Some(token.clone().into());
                self.state.config.insert::<String, StringValue>(
                    StateKey::Client(ClientKey::Token),
                    token.into(),
                );
                Ok(())
            }
            Err(_) => Err(Error::Login),
        }
    }

    /// Retrieve a list of the user's playlists
    pub async fn user_playlists(&self) -> Result<UserPlaylists> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::UserPlaylist.as_str());
        let params = vec![("limit", "500"), ("extra", "tracks"), ("offset", "0")];

        call!(self, endpoint, Some(params))
    }

    /// Retrieve a playlist
    pub async fn playlist(&self, playlist_id: String) -> Result<Playlist> {
        let endpoint = format!("{}{}", self.base_url, Endpoint::Playlist.as_str());
        let params = vec![
            ("limit", "500"),
            ("extra", "tracks"),
            ("playlist_id", playlist_id.as_str()),
            ("offset", "0"),
        ];

        call!(self, endpoint, Some(params))
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
            StringValue::from(secret)
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
    fn set_token(&mut self, token: StringValue) {
        self.user_token = Some(token);
    }

    // Set a username for authentication
    fn set_username(&mut self, username: StringValue) {
        self.username = Some(username);
    }

    // Set a password for authentication
    fn set_password(&mut self, password: StringValue) {
        self.password = Some(password);
    }

    // Set an app_id for authentication
    fn set_app_id(&mut self, app_id: Option<StringValue>) {
        self.app_id = app_id;
    }

    // Set an app secret for authentication
    fn set_active_secret(&mut self, active_secret: Option<StringValue>) {
        self.active_secret = active_secret;
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
        }

        if let Some(token) = &self.user_token {
            info!("adding token to request headers: {}", token);
            headers.insert(
                "X-User-Auth-Token",
                HeaderValue::from_str(token.as_str()).unwrap(),
            );
        }

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
            StatusCode::BAD_REQUEST => Err(Error::API {
                message: "Bad request".to_string(),
            }),
            StatusCode::UNAUTHORIZED => Err(Error::API {
                message: "Unauthorized request".to_string(),
            }),
            StatusCode::NOT_FOUND => Err(Error::API {
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
            let app_id: StringValue = captures
                .name("app_id")
                .map_or("".to_string(), |m| m.as_str().to_string())
                .into();

            self.app_id = Some(app_id.clone());
            self.state
                .config
                .insert::<String, StringValue>(StateKey::Client(ClientKey::AppID), app_id.clone());

            let seed_data = self.seed_regex.captures_iter(bundle_contents.as_str());

            seed_data.for_each(|s| {
                let seed = s.name("seed").map_or("", |m| m.as_str()).to_string();
                let timezone = s.name("timezone").map_or("", |m| m.as_str()).to_string();

                let info_regex = format!(format_info!(), crate::capitalize(&timezone));
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
                self.state.config.insert::<String, StringValue>(
                    StateKey::Client(ClientKey::ActiveSecret),
                    secret_string.clone().into(),
                );
                active_secret = Some(secret_string);

                break;
            };
        }

        if let Some(secret) = active_secret {
            self.set_active_secret(Some(secret.into()));
            Ok(())
        } else {
            Err(Error::ActiveSecret)
        }
    }
}

macro_rules! output {
    ($results:ident, $output_format:expr) => {
        match $output_format {
            Some(OutputFormat::JSON) => {
                let json =
                    serde_json::to_string(&$results).expect("failed to convert results to string");

                print!("{}", json);
            }
            Some(OutputFormat::TSV) => {
                let formatted_results: Vec<Vec<String>> = $results.into();

                let rows = formatted_results
                    .iter()
                    .map(|row| {
                        let tabbed = row.join("\t");

                        tabbed
                    })
                    .collect::<Vec<String>>();

                print!("{}", rows.join("\n"));
            }
            None => {
                let mut table = Table::new();
                table.load_preset(UTF8_FULL);
                table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
                table.set_header($results.table_headers());

                let table_rows: Vec<Vec<String>> = $results.into();

                for row in table_rows {
                    table.add_row(row);
                }

                print!("{}", table);
            }
        }
    };
}

pub(crate) use output;

#[derive(Clone, Debug, Serialize, Deserialize, ValueEnum)]
pub enum OutputFormat {
    JSON,
    #[clap(name = "tabs")]
    TSV,
}
