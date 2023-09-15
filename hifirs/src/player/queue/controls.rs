use crate::{
    player,
    player::queue::{TrackListType, TrackListValue},
    qobuz,
    service::{Album, MusicService, Playlist, SearchResults, Track, TrackStatus},
    sql::db,
};
use futures::executor;
use gstreamer::{ClockTime, State as GstState};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::{
    broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender},
    RwLock,
};

#[derive(Debug, Clone)]
pub struct PlayerState {
    service: Arc<dyn MusicService>,
    current_track: Option<Track>,
    tracklist: TrackListValue,
    status: GstState,
    resume: bool,
    target_status: GstState,
    quit_sender: BroadcastSender<bool>,
}

pub type SafePlayerState = Arc<RwLock<PlayerState>>;

#[derive(Debug, Clone, Default)]
pub struct SavedState {
    pub rowid: i64,
    pub playback_track_id: i64,
    pub playback_position: i64,
    pub playback_track_index: i64,
    pub playback_entity_id: String,
    pub playback_entity_type: String,
}

impl From<PlayerState> for SavedState {
    fn from(state: PlayerState) -> Self {
        if let Some(current_track) = state.current_track() {
            let playback_track_index = current_track.position as i64;
            let playback_track_id = current_track.id as i64;
            let playback_position = player::position().unwrap_or_default().mseconds() as i64;
            let playback_entity_type = state.list_type();
            let playback_entity_id = match playback_entity_type {
                TrackListType::Album => state.album().expect("failed to get album id").id.clone(),
                TrackListType::Playlist => state
                    .playlist()
                    .expect("failed to get playlist id")
                    .id
                    .to_string(),
                TrackListType::Track => current_track.id.to_string(),
                TrackListType::Unknown => "".to_string(),
            };

            Self {
                rowid: 0,
                playback_position,
                playback_track_id,
                playback_entity_id,
                playback_entity_type: playback_entity_type.to_string(),
                playback_track_index,
            }
        } else {
            Self::default()
        }
    }
}

impl PlayerState {
    pub async fn play_album(&mut self, album_id: String) -> Option<String> {
        if let Some(album) = self.service.album(album_id.as_str()).await {
            let mut tracklist = TrackListValue::new(Some(album.tracks.clone()));
            tracklist.set_album(album);
            tracklist.set_list_type(TrackListType::Album);
            tracklist.set_track_status(1, TrackStatus::Playing);

            self.replace_list(tracklist.clone());

            if let Some(mut entry) = tracklist.queue.first_entry() {
                let first_track = entry.get_mut();

                self.attach_track_url(first_track).await;
                self.set_current_track(first_track.clone());
                self.set_target_status(GstState::Playing);

                first_track.track_url.clone()
            } else {
                None
            }
        } else {
            None
        }
    }
    pub async fn play_track(&mut self, track_id: i32) -> Option<String> {
        if let Some(mut track) = self.service.track(track_id).await {
            track.status = TrackStatus::Playing;
            track.number = 1;

            let mut queue = BTreeMap::new();
            queue.entry(track.position).or_insert_with(|| track.clone());

            let mut tracklist = TrackListValue::new(Some(queue));
            tracklist.set_list_type(TrackListType::Track);

            self.replace_list(tracklist.clone());

            self.attach_track_url(&mut track).await;
            self.set_current_track(track.clone());
            self.set_target_status(GstState::Playing);

            track.track_url.clone()
        } else {
            None
        }
    }
    pub async fn play_playlist(&mut self, playlist_id: i64) -> Option<String> {
        if let Some(playlist) = self.service.playlist(playlist_id).await {
            let mut tracklist = TrackListValue::new(Some(playlist.tracks.clone()));

            tracklist.set_playlist(playlist);
            tracklist.set_list_type(TrackListType::Playlist);
            tracklist.set_track_status(1, TrackStatus::Playing);

            self.replace_list(tracklist.clone());

            if let Some(mut entry) = tracklist.queue.first_entry() {
                let first_track = entry.get_mut();

                self.attach_track_url(first_track).await;
                self.set_current_track(first_track.clone());
                self.set_target_status(GstState::Playing);

                first_track.track_url.clone()
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn set_status(&mut self, status: GstState) {
        self.status = status;
    }

    pub fn status(&self) -> GstState {
        self.status
    }

    pub fn set_current_track(&mut self, track: Track) {
        self.current_track = Some(track);
    }

    pub fn set_resume(&mut self, resume: bool) {
        self.resume = resume;
    }

    pub fn resume(&self) -> bool {
        self.resume
    }

    pub fn current_track(&self) -> Option<Track> {
        self.current_track.clone()
    }

    pub fn current_track_position(&self) -> u32 {
        if let Some(track) = &self.current_track {
            track.position
        } else {
            0
        }
    }

    pub fn unplayed_tracks(&self) -> Vec<&Track> {
        self.tracklist.unplayed_tracks()
    }

    pub fn played_tracks(&self) -> Vec<&Track> {
        self.tracklist.played_tracks()
    }

    pub fn album(&self) -> Option<&Album> {
        self.tracklist.get_album()
    }

    pub fn list_type(&self) -> TrackListType {
        self.tracklist.list_type.clone()
    }

    pub fn playlist(&self) -> Option<&Playlist> {
        self.tracklist.get_playlist()
    }

    pub fn replace_list(&mut self, tracklist: TrackListValue) {
        debug!("replacing tracklist");
        self.tracklist = tracklist;
    }

    pub fn track_list(&self) -> TrackListValue {
        self.tracklist.clone()
    }

    pub fn set_track_status(&mut self, position: u32, status: TrackStatus) {
        self.tracklist.set_track_status(position, status);
    }

    pub fn target_status(&self) -> GstState {
        self.target_status
    }

    pub fn set_target_status(&mut self, target: GstState) {
        self.target_status = target;
    }

    /// Attach a `TrackURL` to the given track.
    pub async fn attach_track_url(&mut self, track: &mut Track) {
        debug!("fetching track url");
        if let Some(track_url) = self.service.track_url(track.id as i32).await {
            debug!("attaching url information to track");
            track.track_url = Some(track_url);
        }
    }

    pub async fn skip_track(&mut self, index: u32) -> Option<Track> {
        let mut current_track = None;

        for t in self.tracklist.queue.values_mut() {
            match t.position.cmp(&index) {
                std::cmp::Ordering::Less => {
                    t.status = TrackStatus::Played;
                }
                std::cmp::Ordering::Equal => {
                    if let Some(track_url) = self.service.track_url(t.id as i32).await {
                        t.status = TrackStatus::Playing;
                        t.track_url = Some(track_url);
                        self.current_track = Some(t.clone());
                        current_track = Some(t.clone());
                    } else {
                        t.status = TrackStatus::Unplayable;
                    }
                }
                std::cmp::Ordering::Greater => {
                    t.status = TrackStatus::Unplayed;
                }
            }
        }

        current_track
    }

    pub async fn search_all(&self, query: &str) -> Option<SearchResults> {
        self.service.search(query).await
    }

    pub async fn fetch_artist_albums(&self, artist_id: i32) -> Option<Vec<Album>> {
        match self.service.artist(artist_id).await {
            Some(results) => results.albums,
            None => None,
        }
    }

    pub async fn fetch_playlist_tracks(&self, playlist_id: i64) -> Option<Vec<Track>> {
        match self.service.playlist(playlist_id).await {
            Some(results) => Some(results.tracks.values().cloned().collect::<Vec<Track>>()),
            None => None,
        }
    }

    pub async fn fetch_user_playlists(&self) -> Option<Vec<Playlist>> {
        self.service.user_playlists().await
    }

    pub fn quitter(&self) -> BroadcastReceiver<bool> {
        self.quit_sender.subscribe()
    }

    pub fn quit(&self) {
        executor::block_on(self.persist());

        self.quit_sender
            .send(true)
            .expect("failed to send quit message");
    }

    pub fn reset(&mut self) {
        self.tracklist.clear();
        self.current_track = None;
        self.status = gstreamer::State::Null;
        self.resume = false;
    }

    pub async fn new(username: Option<&str>, password: Option<&str>) -> Self {
        let client = Arc::new(
            qobuz::make_client(username, password)
                .await
                .expect("error making client"),
        );

        let tracklist = TrackListValue::new(None);
        let (quit_sender, _) = tokio::sync::broadcast::channel::<bool>(1);

        Self {
            current_track: None,
            service: client,
            tracklist,
            status: gstreamer::State::Null,
            target_status: gstreamer::State::Null,
            resume: false,
            quit_sender,
        }
    }

    pub async fn persist(&self) {
        debug!("persisting state to database");
        if self.current_track.is_some() {
            db::persist_state(self.clone()).await;
        }
    }

    pub async fn load_last_state(&mut self) -> Option<ClockTime> {
        if let Some(last_state) = db::get_last_state().await {
            let entity_type: TrackListType = last_state.playback_entity_type.as_str().into();

            match entity_type {
                TrackListType::Album => {
                    if let Some(album) = self.service.album(&last_state.playback_entity_id).await {
                        self.replace_list(TrackListValue::new(Some(album.tracks.clone())));
                        self.tracklist.set_list_type(TrackListType::Album);
                        self.tracklist.set_album(album);

                        self.skip_track(last_state.playback_track_index as u32)
                            .await;

                        let position =
                            ClockTime::from_mseconds(last_state.playback_position as u64);
                        return Some(position);
                    }
                }
                TrackListType::Playlist => {
                    if let Some(playlist) = self
                        .service
                        .playlist(
                            last_state
                                .playback_entity_id
                                .parse::<i64>()
                                .expect("failed to parse integer"),
                        )
                        .await
                    {
                        self.replace_list(TrackListValue::new(Some(playlist.tracks.clone())));
                        self.tracklist.set_list_type(TrackListType::Playlist);
                        self.tracklist.set_playlist(playlist);

                        self.skip_track(last_state.playback_track_index as u32)
                            .await;

                        let position =
                            ClockTime::from_mseconds(last_state.playback_position as u64);
                        return Some(position);
                    }
                }
                TrackListType::Track => {
                    let track_id: i32 = last_state
                        .playback_entity_id
                        .parse()
                        .expect("failed to parse track id");
                    if let Some(mut track) = self.service.track(track_id).await {
                        track.status = TrackStatus::Playing;
                        track.number = 1;

                        let mut queue = BTreeMap::new();
                        queue.entry(track.position).or_insert_with(|| track);

                        let mut tracklist = TrackListValue::new(Some(queue));
                        tracklist.set_list_type(TrackListType::Track);

                        self.replace_list(tracklist);
                        self.tracklist.set_list_type(TrackListType::Track);

                        self.skip_track(last_state.playback_track_index as u32)
                            .await;

                        let position =
                            ClockTime::from_mseconds(last_state.playback_position as u64);
                        return Some(position);
                    }
                }
                TrackListType::Unknown => unreachable!(),
            }
        }

        None
    }
}
