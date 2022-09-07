use futures::stream::TryStreamExt;
use rspotify::{
    model::{FullPlaylist, FullTrack, PlaylistId, PlaylistItem, SimplifiedPlaylist},
    prelude::*,
    scopes, AuthCodeSpotify, Config, Credentials as SpotifyCredentials, OAuth,
};
use snafu::prelude::*;
use std::{path::PathBuf, str::FromStr};

use crate::Isrc;

#[allow(unused)]
pub struct SpotifyFullPlaylist {
    spotify_playlist: FullPlaylist,
    all_items: Vec<PlaylistItem>,
}

pub struct Spotify {
    client: AuthCodeSpotify,
}

pub async fn new() -> Spotify {
    let creds = SpotifyCredentials::from_env().unwrap();

    // Using every possible scope
    let scopes = scopes!(
        "user-library-read",
        "playlist-read-collaborative",
        "playlist-read-private",
        "playlist-modify-public",
        "playlist-modify-private"
    );
    let oauth = OAuth::from_env(scopes).unwrap();

    let config = Config {
        cache_path: PathBuf::from_str("/tmp/.spotify_token_cache.json").unwrap(),
        token_cached: true,
        token_refreshing: true,
        ..Default::default()
    };

    let mut client = AuthCodeSpotify::with_config(creds.clone(), oauth.clone(), config);

    let url = client.get_authorize_url(true).unwrap();
    // This function requires the `cli` feature enabled.
    client.prompt_for_token(&url).await.unwrap();

    Spotify { client }
}

impl Spotify {
    pub async fn user_playlists(&self) -> Vec<SimplifiedPlaylist> {
        let mut playlists = self.client.current_user_playlists();
        let mut all_playlists: Vec<SimplifiedPlaylist> = vec![];

        while let Some(list) = playlists.try_next().await.unwrap() {
            all_playlists.push(list);
        }

        all_playlists
    }

    pub async fn playlist(&self, playlist_id: PlaylistId) -> Result<SpotifyFullPlaylist> {
        let spotify_playlist = self.client.playlist(&playlist_id, None, None).await?;

        let mut items = self.client.playlist_items(&playlist_id, None, None);
        let mut all_items: Vec<PlaylistItem> = vec![];

        while let Some(item) = items.try_next().await.unwrap() {
            all_items.push(item);
        }

        Ok(SpotifyFullPlaylist {
            spotify_playlist,
            all_items,
        })
    }
}

impl SpotifyFullPlaylist {
    pub fn isrc_list(&self) -> Vec<Isrc> {
        self.all_items
            .iter()
            .filter_map(|playlist_item| {
                if let Some(playable_item) = &playlist_item.track {
                    match playable_item {
                        rspotify::model::PlayableItem::Track(track) => track
                            .external_ids
                            .get("isrc")
                            .map(|isrc| Isrc(isrc.to_string())),
                        rspotify::model::PlayableItem::Episode(_) => None,
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<Isrc>>()
    }

    pub fn missing_tracks(&self, isrcs: Vec<Isrc>) -> Vec<FullTrack> {
        self.all_items
            .iter()
            .cloned()
            .filter_map(|playlist_item| {
                if let Some(playable_item) = playlist_item.track {
                    match playable_item {
                        rspotify::model::PlayableItem::Track(track) => {
                            if let Some(isrc) = track.external_ids.get("isrc") {
                                if isrcs.contains(&Isrc(isrc.to_string())) {
                                    None
                                } else {
                                    Some(track)
                                }
                            } else {
                                None
                            }
                        }
                        rspotify::model::PlayableItem::Episode(_) => None,
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<FullTrack>>()
    }

    pub fn track_count(&self) -> usize {
        self.all_items.len() as usize
    }
}

#[derive(Snafu, Debug)]
pub enum Error {
    ClientError { error: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<rspotify::ClientError> for Error {
    fn from(error: rspotify::ClientError) -> Self {
        Error::ClientError {
            error: error.to_string(),
        }
    }
}
