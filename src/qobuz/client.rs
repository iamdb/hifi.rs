use std::collections::HashMap;
use std::io::Read;

use reqwest::blocking::Response;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Method, StatusCode};
use serde_json::Value;

use crate::utils;

use super::{Album, AlbumSearchResults, Artist, ArtistSearchResults, TrackInfo, TrackURL};

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

#[derive(Debug)]
pub struct Client {
    secrets: HashMap<String, String>,
    active_secret: Option<String>,
    app_id: Option<String>,
    username: Option<String>,
    password: Option<String>,
    cookie: Option<String>,
    base_url: String,
    client: reqwest::blocking::Client,
    user_token: Option<String>,
    bundle_regex: regex::Regex,
    app_id_regex: regex::Regex,
    seed_regex: regex::Regex,
}

pub fn new() -> Client {
    let client = reqwest::blocking::Client::new();

    Client {
        client,
        secrets: HashMap::new(),
        active_secret: None,
        user_token: None,
        app_id: None,
        cookie: None,
        username: None,
        password: None,
        base_url: "https://www.qobuz.com/api.json/0.2/".to_string(),
        bundle_regex: regex::Regex::new(BUNDLE_REGEX).unwrap(),
        app_id_regex: regex::Regex::new(APP_REGEX).unwrap(),
        seed_regex: regex::Regex::new(SEED_REGEX).unwrap(),
    }
}

enum APIEndpoints {
    Album,
    Artist,
    Label,
    Login,
    Track,
    UserPlaylist,
    SearchArtists,
    SearchAlbums,
    TrackURL,
}

impl APIEndpoints {
    fn as_str(&self) -> &'static str {
        match self {
            APIEndpoints::Album => "album/get",
            APIEndpoints::Artist => "artist/get",
            APIEndpoints::Label => "label/get",
            APIEndpoints::Login => "user/login",
            APIEndpoints::Track => "track/get",
            APIEndpoints::SearchArtists => "artist/search",
            APIEndpoints::UserPlaylist => "playlist/getUserPlaylists",
            APIEndpoints::SearchAlbums => "album/search",
            APIEndpoints::TrackURL => "track/getFileUrl",
        }
    }
}

#[allow(dead_code)]
impl Client {
    pub fn login(&mut self) -> Option<String> {
        let endpoint = format!("{}{}", self.base_url, APIEndpoints::Login.as_str());
        let app_id = self.app_id.as_ref().unwrap().to_string();
        let username = self.username.as_ref().unwrap().to_string();
        let password = self.password.as_ref().unwrap().to_string();

        let params = vec![
            ("email", username),
            ("password", password),
            ("app_id", app_id),
        ];

        if let Ok(response) = self.make_call(endpoint, Some(params)) {
            let json: Value = serde_json::from_str(response.as_str()).unwrap();
            info!("Successfully logged in");
            debug!("{}", json);
            let mut token = json["user_auth_token"].to_string();
            token = token[1..token.len() - 1].to_string();

            self.user_token = Some(token.clone());
            Some(token)
        } else {
            None
        }
    }

    pub fn user_playlists(&mut self) {
        let endpoint = format!("{}{}", self.base_url, APIEndpoints::UserPlaylist.as_str());
        let params = vec![("limit", "500".to_string())];

        if let Ok(response) = self.make_call(endpoint, Some(params)) {
            debug!("{}", response);
        }
    }

    pub fn track(&mut self, track_id: &str) -> Option<TrackInfo> {
        let endpoint = format!("{}{}", self.base_url, APIEndpoints::Track.as_str());
        let params = vec![("track_id", track_id.to_string())];

        if let Ok(response) = self.make_call(endpoint, Some(params)) {
            let track_info: TrackInfo = serde_json::from_str(response.as_str()).unwrap();
            Some(track_info)
        } else {
            None
        }
    }

    pub fn track_url(
        &mut self,
        track_id: &i32,
        fmt_id: &i32,
        sec: Option<String>,
    ) -> Result<TrackURL, String> {
        let endpoint = format!("{}{}", self.base_url, APIEndpoints::TrackURL.as_str());
        let now = format!("{}", chrono::Utc::now().timestamp());
        let mut secret = self
            .active_secret
            .as_ref()
            .unwrap_or(&"".to_string())
            .to_string();

        if let Some(s) = sec {
            secret = s;
        }

        let sig = format!(
            "trackgetFileUrlformat_id{}intentstreamtrack_id{}{}{}",
            fmt_id, track_id, now, secret
        );
        let hashed_sig = format!("{:x}", md5::compute(sig.as_str()));
        let params = vec![
            ("request_ts", now),
            ("request_sig", hashed_sig),
            ("track_id", track_id.to_string()),
            ("format_id", fmt_id.to_string()),
            ("intent", "stream".to_string()),
        ];

        match self.make_call(endpoint, Some(params)) {
            Ok(response) => {
                let track_url: TrackURL = serde_json::from_str(response.as_str()).unwrap();
                Ok(track_url)
            }
            Err(response) => Err(response),
        }
    }

    pub fn album(&mut self, album_id: &str) -> Option<Album> {
        let endpoint = format!("{}{}", self.base_url, APIEndpoints::Album.as_str());
        let params = vec![("album_id", album_id.to_string())];

        if let Ok(response) = self.make_call(endpoint, Some(params)) {
            let album: Album = serde_json::from_str(response.as_str()).unwrap();
            Some(album)
        } else {
            None
        }
    }

    pub fn search_albums(&mut self, query: &str) -> Option<AlbumSearchResults> {
        let endpoint = format!("{}{}", self.base_url, APIEndpoints::SearchAlbums.as_str());
        let params = vec![("query", query.to_string()), ("limit", "500".to_string())];

        if let Ok(response) = self.make_call(endpoint, Some(params)) {
            let results: AlbumSearchResults = serde_json::from_str(response.as_str()).unwrap();
            Some(results)
        } else {
            None
        }
    }

    pub fn artist(&mut self, artist_id: &str) -> Option<Artist> {
        let endpoint = format!("{}{}", self.base_url, APIEndpoints::Artist.as_str());
        let params = vec![
            ("artist_id", artist_id.to_string()),
            ("app_id", self.app_id.as_ref().unwrap().to_string()),
            ("limit", "500".to_string()),
            ("offset", "0".to_string()),
            ("extra", "albums".to_string()),
        ];

        if let Ok(response) = self.make_call(endpoint, Some(params)) {
            let artist: Artist = serde_json::from_str(response.as_str()).unwrap();
            Some(artist)
        } else {
            None
        }
    }

    pub fn search_artists(&mut self, query: &str) -> Option<ArtistSearchResults> {
        let endpoint = format!("{}{}", self.base_url, APIEndpoints::SearchArtists.as_str());
        let params = vec![("query", query.to_string()), ("limit", "500".to_string())];

        if let Ok(response) = self.make_call(endpoint, Some(params)) {
            let results: ArtistSearchResults = serde_json::from_str(response.as_str()).unwrap();
            Some(results)
        } else {
            None
        }
    }

    pub fn label(&mut self, label_id: &str) {
        let endpoint = format!("{}{}", self.base_url, APIEndpoints::Label.as_str());
        let params = vec![
            ("label_id", label_id.to_string()),
            ("limit", "500".to_string()),
            ("offset", "0".to_string()),
            ("extra", "albums".to_string()),
        ];

        if let Ok(response) = self.make_call(endpoint, Some(params)) {
            debug!("res:\t{}", response);
        }
    }

    pub fn set_token(&mut self, token: String) {
        self.user_token = Some(token);
    }

    pub fn set_username(&mut self, username: String) {
        self.username = Some(username);
    }

    pub fn set_password(&mut self, password: String) {
        self.password = Some(password);
    }

    pub fn check_auth(&mut self) {
        if self.app_id.is_none() {
            self.get_config();
        }
        if self.user_token.is_some() {
            return;
        }
        if self.username.is_some() && self.password.is_some() {
            self.login();
            self.test_secrets();
        } else {
            panic!("Username and password required.");
        }
    }

    fn make_call(
        &mut self,
        endpoint: String,
        params: Option<Vec<(&str, String)>>,
    ) -> Result<String, String> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "User-Agent",
            HeaderValue::from_str(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:83.0) Gecko/20100101 Firefox/83.0",
            )
            .unwrap(),
        );

        if let Some(app_id) = self.app_id.as_ref() {
            debug!("adding app_id ({}) to headers", app_id);
            headers.insert("X-App-Id", HeaderValue::from_str(app_id.as_str()).unwrap());
        }

        if let Some(cookie) = self.cookie.as_ref() {
            debug!("adding cookie to headers");
            headers.insert("Cookie", HeaderValue::from_str(cookie.as_str()).unwrap());
        }

        if let Some(token) = &self.user_token {
            headers.insert(
                "X-User-Auth-Token",
                HeaderValue::from_str(token.as_str()).unwrap(),
            );
        }

        let request = self.client.request(Method::GET, endpoint).headers(headers);

        if let Some(p) = params {
            let response = request.query(&p).send();
            match response {
                Ok(r) => self.handle_response(r),
                Err(err) => Err(err.to_string()),
            }
        } else {
            let response = request.send();
            match response {
                Ok(r) => self.handle_response(r),
                Err(err) => Err(err.to_string()),
            }
        }
    }

    fn handle_response(&mut self, response: Response) -> Result<String, String> {
        match response.status() {
            StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::NOT_FOUND => {
                let res = response.text().unwrap();
                error!("{}", res);
                Err(res)
            }
            StatusCode::OK => {
                if let Some(cookie) = response.headers().get("set-cookie") {
                    debug!("new cookie from server {:?}", cookie);
                    self.cookie = Some(cookie.to_str().unwrap().to_string());
                }
                let res = response.text().unwrap();
                Ok(res)
            }
            _ => unreachable!(),
        }
    }

    fn get_config(&mut self) {
        let play_url = "https://play.qobuz.com";
        let mut login_page = self
            .client
            .get(format!("{}/login", play_url))
            .send()
            .unwrap();

        let mut contents = "".to_string();
        login_page.read_to_string(&mut contents).unwrap();

        let bundle_path = self
            .bundle_regex
            .captures(contents.as_str())
            .unwrap()
            .get(1)
            .map_or("", |m| m.as_str());

        let bundle_url = format!("{}{}", play_url, bundle_path);
        let mut bundle_page = self.client.get(bundle_url).send().unwrap();

        let mut bundle_contents = "".to_string();
        bundle_page.read_to_string(&mut bundle_contents).unwrap();

        let app_id = self
            .app_id_regex
            .captures(bundle_contents.as_str())
            .unwrap()
            .name("app_id")
            .map_or("", |m| m.as_str());

        let seed_data = self.seed_regex.captures_iter(bundle_contents.as_str());

        seed_data.for_each(|s| {
            let seed = s.name("seed").map_or("", |m| m.as_str()).to_string();
            let timezone = s.name("timezone").map_or("", |m| m.as_str()).to_string();

            let info_regex = format!(format_info!(), utils::capitalize(&timezone));
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
                    let decoded_secret = base64::decode(encoded_secret).unwrap();
                    let secret_utf8 = std::str::from_utf8(&decoded_secret).unwrap().to_string();

                    debug!("{}\t{}\t{}", app_id, timezone.to_lowercase(), secret_utf8);

                    self.app_id = Some(app_id.to_string());
                    self.secrets.insert(timezone, secret_utf8);
                });
        });
    }

    pub fn test_secrets(&mut self) {
        debug!("testing secrets");
        let secrets = self.secrets.clone();

        for (_, secret) in secrets.iter() {
            let response = self.track_url(&5966783, &5, Some(secret.to_string()));

            if response.is_ok() {
                self.active_secret = Some(secret.to_string());
            }
        }
    }
}
