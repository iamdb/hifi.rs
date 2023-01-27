use crate::Isrc;
use futures::stream::TryStreamExt;
use indicatif::ProgressBar;
use log::debug;
use rspotify::{
    model::{FullPlaylist, FullTrack, PlayableItem, PlaylistId, PlaylistItem, SimplifiedPlaylist},
    prelude::*,
    scopes, AuthCodeSpotify, Config, Credentials as SpotifyCredentials, OAuth,
};
use snafu::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    str::FromStr,
};
use tokio::select;
use warp::Filter;

const TOKEN_CACHE: &str = "/tmp/.spotify_token_cache.json";

#[allow(unused)]
pub struct SpotifyFullPlaylist {
    spotify_playlist: FullPlaylist,
    all_tracks: Vec<FullTrack>,
}

pub struct Spotify<'s> {
    client: AuthCodeSpotify,
    progress: &'s ProgressBar,
}

pub async fn new<'s>(progress: &'_ ProgressBar) -> Spotify<'_> {
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

    let client = AuthCodeSpotify::with_config(creds, oauth, config);

    Spotify { client, progress }
}

impl<'s> Spotify<'s> {
    pub async fn auth(&mut self) {
        self.progress.set_message("signing into Spotify");
        if let Ok(Some(token)) = self.client.read_token_cache(true).await {
            debug!("found token in cache: {:?}", token);
            let expired = token.is_expired();

            *self.client.get_token().lock().await.unwrap() = Some(token);

            if expired {
                match self
                    .client
                    .refetch_token()
                    .await
                    .expect("failed to refetch token")
                {
                    Some(refreshed_token) => {
                        debug!("cached token refreshed");
                        *self.client.get_token().lock().await.unwrap() = Some(refreshed_token);
                    }
                    None => {
                        debug!("no cached token, getting auth url");
                        let url = self.client.get_authorize_url(true).unwrap();

                        if webbrowser::open(&url).is_ok() {
                            self.wait_for_auth().await;
                        } else {
                            println!("There was a problem opening the browser, please open this url manually:\n{url}");
                            self.wait_for_auth().await;
                        }
                    }
                }
            }
        } else {
            debug!("no cached token, getting auth url");
            let url = self.client.get_authorize_url(true).unwrap();

            if webbrowser::open(&url).is_ok() {
                self.wait_for_auth().await;
            } else {
                println!(
                    "There was a problem opening the browser, please open this url manually:\n{url}",
                );
                self.wait_for_auth().await;
            }
        }
        self.progress.set_message("signed into Spotify");
    }

    pub async fn wait_for_auth(&mut self) {
        let (tx, rx) = flume::bounded::<String>(1);

        let oauth_callback = warp::path!("callback")
            .and(warp::get())
            .and(warp::query::<HashMap<String, String>>())
            .map(move |mut qs: HashMap<String, String>| {
                if let Some(oauth_code) = qs.remove("code") {
                    tx.send(oauth_code).expect("failed to send code");
                }
                "you can close this"
            });

        debug!("creating temp http server for auth callback");
        self.progress.set_message("waiting for authorization");
        let server_handle = tokio::spawn(warp::serve(oauth_callback).run(([127, 0, 0, 1], 8080)));

        loop {
            select! {
                Ok(code) = rx.recv_async() => {
                    debug!("received code: {}", code);

                    self.client.request_token(code.as_str()).await.expect("failed to get auth token");
                    server_handle.abort();
                    break;
                }
            }
        }

        self.progress.set_message("authorization received");
    }

    pub async fn user_playlists(&self) -> Vec<SimplifiedPlaylist> {
        self.progress.set_message("fetching user's playlists");
        let mut playlists = self.client.current_user_playlists();
        let mut all_playlists: Vec<SimplifiedPlaylist> = vec![];

        while let Some(list) = playlists.try_next().await.unwrap() {
            all_playlists.push(list);
        }

        self.progress.set_message("user's playlists retrieved");

        all_playlists
    }

    pub async fn playlist(&self, playlist_id: PlaylistId<'_>) -> Result<SpotifyFullPlaylist> {
        self.progress
            .set_message(format!("fetching playlist: {playlist_id}"));
        debug!("fetching playlist {}", playlist_id.to_string());

        let spotify_playlist = self
            .client
            .playlist(playlist_id.clone(), None, None)
            .await?;

        debug!("fetching all spotify playlist items");
        let items = self.client.playlist_items(playlist_id, None, None);

        match items.try_collect::<Vec<PlaylistItem>>().await {
            Ok(full_list) => {
                debug!("list size: {}", full_list.len());

                self.progress.set_message("playlist tracks retrieved");

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
                .map(|isrc| set.insert(Isrc(isrc.to_lowercase())));
        }

        set
    }

    pub fn missing_tracks(&self, isrcs: HashSet<Isrc>) -> Vec<MissingTrack> {
        let spotify_isrcs = self.isrc_list();
        let diff = spotify_isrcs.difference(&isrcs).collect::<HashSet<_>>();

        self.all_tracks
            .iter()
            .cloned()
            .enumerate()
            .filter_map(|(index, track)| {
                if let Some(isrc) = track.external_ids.get("isrc") {
                    if diff.contains::<Isrc>(&isrc.into()) {
                        Some(MissingTrack { track, index })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<MissingTrack>>()
    }

    pub fn track_count(&self) -> usize {
        self.all_tracks.len()
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

pub struct MissingTrack {
    pub track: FullTrack,
    pub index: usize,
}
