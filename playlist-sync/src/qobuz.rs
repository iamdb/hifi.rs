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

    pub async fn add_track(&self, playlist_id: String, track_id: String) {
        self.client
            .playlist_add_track(playlist_id, vec![track_id])
            .await
            .expect("failed to add track to playlist");
    }

    pub async fn update_track_position(&self, playlist_id: String, track_id: String, index: usize) {
        self.client
            .playlist_track_position(index, playlist_id, track_id)
            .await
            .expect("failed to update playlist track position");
    }
}

pub struct QobuzPlaylist(Playlist);

impl QobuzPlaylist {
    pub fn irsc_list(&self) -> HashSet<Isrc> {
        if let Some(tracks) = &self.0.tracks {
            let mut set = HashSet::new();

            for track in &tracks.items {
                if let Some(isrc) = &track.isrc {
                    set.insert(Isrc(isrc.to_lowercase()));
                }
            }

            set
        } else {
            HashSet::new()
        }
    }

    pub fn id(&self) -> String {
        self.0.id.to_string()
    }

    pub fn insert(&mut self, index: usize, track: &Track) {
        if let Some(tracks) = self.0.tracks.as_mut() {
            tracks.items.insert(index, track.clone());
        }
    }

    pub fn push(&mut self, track: &Track) {
        if let Some(tracks) = self.0.tracks.as_mut() {
            tracks.items.push(track.clone());
        }
    }

    pub fn track_count(&self) -> usize {
        if let Some(tracks) = &self.0.tracks {
            tracks.items.len() as usize
        } else {
            0
        }
    }
}
