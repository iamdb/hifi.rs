use futures::stream::TryStreamExt;
use rspotify::{
    model::{FullPlaylist, FullTrack, PlaylistId, SimplifiedPlaylist},
    prelude::*,
    scopes, AuthCodeSpotify, Config, Credentials as SpotifyCredentials, OAuth,
};
use snafu::prelude::*;
use std::{path::PathBuf, str::FromStr};

#[derive(Snafu, Debug)]
pub enum Error {
    ClientError { error: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(PartialEq)]
struct Isrc(String);
struct SpotifyFullPlaylist(FullPlaylist);

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
    async fn user_playlists(&self) -> Vec<SimplifiedPlaylist> {
        let mut playlists = self.client.current_user_playlists();
        let mut all_playlists: Vec<SimplifiedPlaylist> = vec![];

        while let Some(list) = playlists.try_next().await.unwrap() {
            all_playlists.push(list);
        }

        all_playlists
    }

    async fn playlist(&self, playlist_id: PlaylistId) -> Result<SpotifyFullPlaylist> {
        match self.client.playlist(&playlist_id, None, None).await {
            Ok(playlist) => Ok(SpotifyFullPlaylist(playlist)),
            Err(err) => Err(Error::ClientError {
                error: err.to_string(),
            }),
        }
    }
}

impl SpotifyFullPlaylist {
    pub fn isrc_list(&self) -> Vec<Isrc> {
        self.0
            .tracks
            .items
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
        self.0
            .tracks
            .items
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
}
