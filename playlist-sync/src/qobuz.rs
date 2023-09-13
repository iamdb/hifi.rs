use crate::Isrc;
use hifirs_qobuz_api::client::{
    api::Client,
    playlist::Playlist,
    track::{Track, Tracks},
};
use indicatif::ProgressBar;
use std::collections::HashSet;

pub struct Qobuz<'q> {
    client: Client,
    progress: &'q ProgressBar,
}

pub async fn new<'q>(progress: &'_ ProgressBar) -> Qobuz<'_> {
    let client = hifirs_qobuz_api::client::api::new(None, None, None, None)
        .await
        .unwrap_or_else(|err| {
            println!("There was a problem creating the api client.");
            println!("Message {err}");
            std::process::exit(1);
        });

    Qobuz { client, progress }
}

impl<'q> Qobuz<'q> {
    pub async fn auth(&mut self, username: &str, password: &str) -> hifirs_qobuz_api::Result<()> {
        self.progress.set_message("signing into Qobuz");
        self.client.refresh().await?;
        self.client.login(username, password).await?;
        self.client.test_secrets().await?;
        self.progress.set_message("signed into Qobuz");

        Ok(())
    }

    pub async fn playlist(&self, playlist_id: i64) -> hifirs_qobuz_api::Result<QobuzPlaylist> {
        self.progress
            .set_message(format!("fetching playlist: {playlist_id}"));

        let playlist = self.client.playlist(playlist_id).await?;

        self.progress.set_message("playlist tracks retrieved");
        Ok(QobuzPlaylist(playlist))
    }

    pub async fn search(&self, query: String) -> Vec<Track> {
        self.progress.set_message(format!("{query} searching"));
        let results = self.client.search_all(query.clone(), 100).await.unwrap();

        if results.tracks.items.is_empty() {
            self.progress.set_message(format!("{query} not found"));
        } else {
            self.progress.set_message(format!("{query} found"));
        }

        results.tracks.items
    }

    pub async fn add_track(&self, playlist_id: String, track_id: String) {
        self.progress
            .set_message(format!("adding {track_id} to {playlist_id}"));
        match self
            .client
            .playlist_add_track(playlist_id.clone(), vec![track_id.clone()])
            .await
        {
            Ok(_) => debug!("track added"),
            Err(error) => error!("failed to add track {}", error.to_string()),
        }
        self.progress
            .set_message(format!("added {track_id} to {playlist_id}"));
    }

    pub async fn update_track_position(
        &self,
        playlist_id: String,
        track_id: String,
        index: usize,
    ) -> hifirs_qobuz_api::Result<()> {
        self.client
            .playlist_track_position(index, playlist_id, track_id)
            .await?;

        Ok(())
    }

    pub async fn delete_track(
        &self,
        playlist_id: String,
        playlist_track_ids: Vec<String>,
    ) -> hifirs_qobuz_api::Result<()> {
        self.client
            .playlist_delete_track(playlist_id, playlist_track_ids)
            .await?;

        Ok(())
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

    pub fn tracks(&self) -> Option<Tracks> {
        self.0.tracks.clone()
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
            tracks.items.len()
        } else {
            0
        }
    }
}
