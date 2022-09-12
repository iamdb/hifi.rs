use crate::{
    get_client,
    state::{
        app::{AppState, ClientKey, StateKey},
        StringValue,
    },
    ui::components::{ColumnWidth, Row, Table, TableHeaders, TableRow, TableRows, TableWidths},
};
use enum_as_inner::EnumAsInner;
use qobuz_client::client::{
    album::{Album, AlbumSearchResults},
    api::Client,
    artist::{Artist, ArtistSearchResults},
    playlist::{Playlist, Playlists, UserPlaylistsResult},
    track::Track,
    AudioQuality,
};
use serde::{Deserialize, Serialize};

pub mod album;
pub mod artist;
pub mod playlist;
pub mod track;

/// Setup app_id, secret and user credentials for authentication
pub async fn setup_client(mut client: Client, app_state: AppState) -> Client {
    info!("setting up the api client");

    let tree = app_state.config.clone();
    let mut refresh_config = false;

    if let Some(app_id) = get_client!(ClientKey::AppID, tree, StringValue) {
        info!("using app_id from cache: {}", app_id);
        client.set_app_id(app_id.to_string());
    } else {
        refresh_config = true;
    }

    if let Some(quality) = get_client!(ClientKey::DefaultQuality, tree, AudioQuality) {
        info!("using default quality from cache: {}", quality);
        client.set_default_quality(quality);
    }

    if let Some(active_secret) = get_client!(ClientKey::ActiveSecret, tree, StringValue) {
        info!("using app_secret from cache: {}", active_secret);
        client.set_active_secret(active_secret.to_string());
    } else {
        refresh_config = true;
    }

    if refresh_config {
        debug!("refreshing app secret and id");
        client.refresh().await;
    }

    if let Some(token) = get_client!(ClientKey::Token, tree, StringValue) {
        info!("using token from cache");
        client.set_token(token.to_string());
    } else if let (Some(username), Some(password)) = (
        get_client!(ClientKey::Username, tree, StringValue),
        get_client!(ClientKey::Password, tree, StringValue),
    ) {
        info!("using username and password from cache");
        client.set_credentials(qobuz_client::client::api::Credentials {
            username: Some(username.to_string()),
            password: Some(password.to_string()),
        });

        info!("signing in");
        client.login().await.expect("failed to login");
    }

    client
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

#[derive(Clone, Debug, Deserialize, Serialize, EnumAsInner)]
pub enum SearchResults {
    Albums(AlbumSearchResults),
    Artists(ArtistSearchResults),
    UserPlaylists(UserPlaylistsResult),
    Playlist(Box<Playlist>),
    Album(Box<Album>),
    Artist(Artist),
}

impl From<SearchResults> for Table {
    fn from(results: SearchResults) -> Self {
        let mut table = Table::new(None, None, None);

        table.set_header(results.headers());
        table.set_rows(results.rows());
        table.set_widths(results.widths());

        table
    }
}

impl From<SearchResults> for Vec<Vec<String>> {
    fn from(results: SearchResults) -> Self {
        match results {
            SearchResults::Albums(r) => r.into(),
            SearchResults::Artists(r) => r.into(),
            SearchResults::UserPlaylists(r) => r.into(),
            SearchResults::Playlist(r) => r.into(),
            SearchResults::Album(r) => r.into(),
            SearchResults::Artist(r) => r.into(),
        }
    }
}

impl From<AlbumSearchResults> for SearchResults {
    fn from(results: AlbumSearchResults) -> Self {
        SearchResults::Albums(results)
    }
}

impl From<Box<Album>> for SearchResults {
    fn from(album: Box<Album>) -> Self {
        Self::Album(album)
    }
}

impl From<SearchResults> for Album {
    fn from(results: SearchResults) -> Self {
        results.into()
    }
}

impl SearchResults {
    pub fn headers(&self) -> Vec<String> {
        match self {
            SearchResults::Albums(_) => Album::headers(),
            SearchResults::Artists(_) => Artist::headers(),
            SearchResults::UserPlaylists(_) => Playlists::headers(),
            SearchResults::Playlist(_) => Track::headers(),
            SearchResults::Album(_) => Album::headers(),
            SearchResults::Artist(_) => Artist::headers(),
        }
    }

    pub fn widths(&self) -> Vec<ColumnWidth> {
        match self {
            SearchResults::Albums(_) => Album::widths(),
            SearchResults::Artists(_) => Artist::widths(),
            SearchResults::UserPlaylists(_) => Playlist::widths(),
            SearchResults::Playlist(_) => Track::widths(),
            SearchResults::Album(_) => Album::widths(),
            SearchResults::Artist(_) => Artist::widths(),
        }
    }

    pub fn rows(&self) -> Vec<Row> {
        match self {
            SearchResults::Albums(r) => r.albums.rows(),
            SearchResults::Artists(r) => r.artists.rows(),
            SearchResults::UserPlaylists(r) => r.playlists.rows(),
            SearchResults::Playlist(r) => r.rows(),
            SearchResults::Album(r) => vec![r.row()],
            SearchResults::Artist(r) => vec![r.row()],
        }
    }
}
