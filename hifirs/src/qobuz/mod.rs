use crate::{
    service::{Album, Artist, MusicService, Playlist, SearchResults, Track},
    sql::db::{self},
};
use async_trait::async_trait;
use hifirs_qobuz_api::{
    client::{
        api::{self, Client as QobuzClient},
        search_results::SearchAllResults,
        AudioQuality,
    },
    Credentials,
};

pub type Result<T, E = hifirs_qobuz_api::Error> = std::result::Result<T, E>;

pub mod album;
pub mod artist;
pub mod playlist;
pub mod track;

#[async_trait]
impl MusicService for QobuzClient {
    async fn login(&self) {
        self.login().await;
    }

    async fn album(&self, album_id: &str) -> Option<Album> {
        match self.album(album_id).await {
            Ok(album) => Some(album.into()),
            Err(_) => None,
        }
    }

    async fn track(&self, track_id: i32) -> Option<Track> {
        match self.track(track_id).await {
            Ok(track) => Some(track.into()),
            Err(_) => None,
        }
    }

    async fn artist(&self, artist_id: i32) -> Option<Artist> {
        match self.artist(artist_id, None).await {
            Ok(track) => Some(track.into()),
            Err(_) => None,
        }
    }

    async fn playlist(&self, playlist_id: i64) -> Option<Playlist> {
        match self.playlist(playlist_id).await {
            Ok(playlist) => Some(playlist.into()),
            Err(_) => None,
        }
    }

    async fn search(&self, query: &str) -> Option<SearchResults> {
        match self.search_all(query.to_string(), 100).await {
            Ok(results) => Some(results.into()),
            Err(_) => None,
        }
    }

    async fn track_url(&self, track_id: i32) -> Option<String> {
        match self.track_url(track_id, None, None).await {
            Ok(track_url) => Some(track_url.url),
            Err(_) => None,
        }
    }

    async fn user_playlists(&self) -> Option<Vec<Playlist>> {
        match self.user_playlists().await {
            Ok(up) => Some(
                up.playlists
                    .items
                    .into_iter()
                    .map(|p| p.into())
                    .collect::<Vec<Playlist>>(),
            ),
            Err(_) => None,
        }
    }
}

pub async fn make_client(
    username: Option<String>,
    password: Option<String>,
) -> Result<QobuzClient> {
    let mut client = api::new(None, None, None, None).await?;
    if username.is_some() || password.is_some() {
        client.set_credentials(Credentials { username, password });
    }

    setup_client(&mut client).await
}

/// Setup app_id, secret and user credentials for authentication
pub async fn setup_client(client: &mut QobuzClient) -> Result<QobuzClient> {
    info!("setting up the api client");

    if let Some(config) = db::get_config().await {
        let mut refresh_config = false;

        if let Some(quality) = config.default_quality {
            info!("using default quality from cache: {}", quality);
            let quality: AudioQuality = quality.into();
            client.set_default_quality(quality);
        }

        if let Some(app_id) = config.app_id {
            debug!("using app_id from cache");
            client.set_app_id(app_id);
        } else {
            debug!("app_id not found, will have to refresh config");
            refresh_config = true;
        }

        if let Some(secret) = config.active_secret {
            debug!("using active secret from cache");
            client.set_active_secret(secret);
        } else {
            debug!("active_secret not found, will have to refresh config");
            refresh_config = true;
        }

        if let Some(token) = config.user_token {
            info!("using token from cache");
            client.set_token(token);

            if refresh_config {
                client.refresh().await?;
                client.test_secrets().await?;

                if let Some(id) = client.get_app_id() {
                    db::set_app_id(id).await;
                }

                if let Some(secret) = client.get_active_secret() {
                    db::set_active_secret(secret).await;
                }
            }
        } else if let (Some(username), Some(password)) = (config.username, config.password) {
            info!("setting auth using username and password from cache");
            client.set_credentials(Credentials {
                username: Some(username),
                password: Some(password),
            });

            if refresh_config {
                client.refresh().await?;

                if let Some(id) = client.get_app_id() {
                    db::set_app_id(id).await;
                }
            }

            client.login().await?;
            client.test_secrets().await?;

            if let Some(token) = client.get_token() {
                db::set_user_token(token).await;
            }

            if let Some(secret) = client.get_active_secret() {
                db::set_active_secret(secret).await;
            }
        } else {
            return Err(hifirs_qobuz_api::Error::NoCredentials);
        }
    }

    Ok(client.clone())
}

impl From<SearchAllResults> for SearchResults {
    fn from(s: SearchAllResults) -> Self {
        Self {
            query: s.query,
            albums: s
                .albums
                .items
                .into_iter()
                .map(|a| a.into())
                .collect::<Vec<Album>>(),
            tracks: s
                .tracks
                .items
                .into_iter()
                .map(|t| t.into())
                .collect::<Vec<Track>>(),
            artists: s
                .artists
                .items
                .into_iter()
                .map(|a| Artist {
                    name: a.name,
                    id: a.id as usize,
                    albums: None,
                })
                .collect::<Vec<Artist>>(),
            playlists: s
                .playlists
                .items
                .into_iter()
                .map(|p| p.into())
                .collect::<Vec<Playlist>>(),
        }
    }
}
