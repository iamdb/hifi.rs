use std::collections::HashSet;

use crate::Isrc;
use qobuz_client::client::{
    api::{Client, Credentials as QobuzCredentials},
    playlist::Playlist,
    track::Track,
};
use snafu::prelude::*;

#[derive(Snafu, Debug)]
pub enum Error {
    ClientError { error: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<qobuz_client::client::api::Error> for Error {
    fn from(error: qobuz_client::client::api::Error) -> Self {
        Error::ClientError {
            error: error.to_string(),
        }
    }
}

pub struct Qobuz {
    client: Client,
}

pub async fn new() -> Qobuz {
    let creds = QobuzCredentials {
        username: Some(env!("QOBUZ_USERNAME").to_string()),
        password: Some(env!("QOBUZ_PASSWORD").to_string()),
    };

    let mut client = qobuz_client::client::api::new(Some(creds.clone()), None, None, None, None)
        .await
        .expect("failed to create client");

    client.refresh().await;
    client.login().await.expect("failed to login");

    Qobuz { client }
}

impl Qobuz {
    pub async fn playlist(&self, playlist_id: String) -> Result<QobuzPlaylist> {
        Ok(QobuzPlaylist(self.client.playlist(playlist_id).await?))
    }

    pub async fn search(&self, query: String) -> Vec<Track> {
        let results = self.client.search_all(query).await.unwrap();

        results.tracks.items
    }
}

pub struct QobuzPlaylist(Playlist);

impl QobuzPlaylist {
    pub fn irsc_list(&self) -> HashSet<Isrc> {
        if let Some(tracks) = &self.0.tracks {
            let mut set = HashSet::new();

            for track in &tracks.items {
                if let Some(isrc) = &track.isrc {
                    set.insert(Isrc(isrc.to_string()));
                }
            }

            set
        } else {
            HashSet::new()
        }
    }

    pub fn track_count(&self) -> usize {
        self.0.tracks_count as usize
    }

    //pub async fn sync_spotify_playlist(&self, spotify_playlist: SpotifyFullPlaylist) {}
}
