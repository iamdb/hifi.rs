use crate::Isrc;
use futures::stream::TryStreamExt;
use log::debug;
use rspotify::{
    model::{FullPlaylist, FullTrack, PlayableItem, PlaylistId, PlaylistItem, SimplifiedPlaylist},
    prelude::*,
    scopes, AuthCodeSpotify, Config, Credentials as SpotifyCredentials, OAuth,
};
use snafu::prelude::*;
use std::{collections::HashSet, path::PathBuf, str::FromStr};
use warp::Filter;

const TOKEN_CACHE: &str = "/tmp/.spotify_token_cache.json";

#[allow(unused)]
pub struct SpotifyFullPlaylist {
    spotify_playlist: FullPlaylist,
    all_tracks: Vec<FullTrack>,
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
        cache_path: PathBuf::from_str(TOKEN_CACHE).expect("failed to create path from TOKEN_CACHE"),
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
        debug!("fetching playlist {}", playlist_id.to_string());
        let spotify_playlist = self.client.playlist(&playlist_id, None, None).await?;

        debug!("fetching all spotify playlist items");
        let items = self.client.playlist_items(&playlist_id, None, None);

        match items.try_collect::<Vec<PlaylistItem>>().await {
            Ok(full_list) => {
                debug!("list size: {}", full_list.len());

                let all_tracks = full_list
                    .iter()
                    .filter_map(|item| {
                        if let Some(playable) = &item.track {
                            match playable {
                                PlayableItem::Track(track) => Some(track),
                                PlayableItem::Episode(_) => {
                                    debug!("skipping episode");
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    })
                    .cloned()
                    .collect::<Vec<FullTrack>>();

                Ok(SpotifyFullPlaylist {
                    spotify_playlist,
                    all_tracks,
                })
            }
            Err(error) => Err(Error::ClientError {
                error: error.to_string(),
            }),
        }
    }
}

impl SpotifyFullPlaylist {
    pub fn isrc_list(&self) -> HashSet<Isrc> {
        let mut set = HashSet::new();

        for track in &self.all_tracks {
            track
                .external_ids
                .get("isrc")
                .map(|isrc| set.insert(Isrc(isrc.to_string())));
        }

        set
    }

    pub fn missing_tracks(&self, isrcs: HashSet<Isrc>) -> Vec<FullTrack> {
        let spotify_isrcs = self.isrc_list();
        let diff = spotify_isrcs.difference(&isrcs).collect::<HashSet<_>>();

        self.all_tracks
            .iter()
            .cloned()
            .filter_map(|track| {
                if let Some(isrc) = track.external_ids.get("isrc") {
                    if diff.contains(&&Isrc(isrc.to_string())) {
                        Some(track)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<FullTrack>>()
    }

    pub fn track_count(&self) -> usize {
        self.all_tracks.len() as usize
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

pub async fn wait_for_auth() {
    let hello = warp::path!("callback" / String).map(|name| format!("Hello, {}!", name));

    warp::serve(hello).run(([127, 0, 0, 1], 8080)).await;
}
