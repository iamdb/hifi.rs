use crate::{
    qobuz::{
        album::Album,
        playlist::Playlist,
        track::{Track, TrackStatus},
    },
    sql::db::Database,
    state::{ClockValue, StatusValue, TrackListType, TrackListValue},
};
use futures::executor;
use gstreamer::{ClockTime, State as GstState};
use hifirs_qobuz_api::client::{api::Client, search_results::SearchAllResults};
use std::{collections::VecDeque, fmt::Display, sync::Arc};
use tokio::sync::{
    broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender},
    RwLock,
};

#[derive(Debug, Clone)]
pub struct PlayerState {
    db: Database,
    client: Client,
    current_track: Option<Track>,
    tracklist: TrackListValue,
    position: ClockValue,
    status: StatusValue,
    resume: bool,
    target_status: StatusValue,
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
        if let (Some(current_track), Some(playback_track_index)) =
            (state.current_track(), state.current_track_index())
        {
            let playback_track_index = playback_track_index as i64;
            let playback_track_id = current_track.id as i64;
            let playback_position = state.position().inner_clocktime().mseconds() as i64;
            let playback_entity_type = state.list_type();
            let playback_entity_id = match playback_entity_type {
                TrackListType::Album => state.album().expect("failed to get album id").id.clone(),
                TrackListType::Playlist => state
                    .playlist()
                    .expect("failed to get playlist id")
                    .id
                    .to_string(),
                TrackListType::Track => state
                    .current_track
                    .expect("failed to get current_track id")
                    .id
                    .to_string(),
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
    pub async fn play_album(
        &mut self,
        album_id: String,
    ) -> (Option<Track>, Option<TrackListValue>) {
        if let Ok(album) = self.client.album(album_id.as_str()).await {
            // if album.tracks.is_empty() {
            //     album.attach_tracks(self.client.clone()).await;
            // }

            let album: Album = album.into();

            let mut tracklist = TrackListValue::new(Some(album.tracks.clone()));
            tracklist.set_album(album);
            tracklist.set_list_type(TrackListType::Album);

            self.replace_list(tracklist.clone());

            let first_track = tracklist.queue.front_mut().unwrap();

            self.attach_track_url(first_track).await;
            self.set_current_track(first_track.clone());
            self.set_target_status(GstState::Playing);

            (Some(first_track.clone()), Some(tracklist))
        } else {
            (None, None)
        }
    }
    pub async fn play_track(&mut self, track_id: i32) -> (Option<Track>, Option<TrackListValue>) {
        if let Ok(new_track) = self.client.track(track_id).await {
            let mut track: Track = new_track.into();
            track.status = TrackStatus::Playing;

            let mut queue = VecDeque::new();
            queue.push_front(track.clone());

            let mut tracklist = TrackListValue::new(Some(queue));
            tracklist.set_list_type(TrackListType::Track);

            self.replace_list(tracklist.clone());

            self.attach_track_url(&mut track).await;
            self.set_current_track(track.clone());
            self.set_target_status(GstState::Playing);

            (Some(track), Some(tracklist))
        } else {
            (None, None)
        }
    }
    pub async fn play_playlist(
        &mut self,
        playlist_id: i64,
    ) -> (Option<Track>, Option<TrackListValue>) {
        if let Ok(playlist) = self.client.playlist(playlist_id).await {
            let playlist: Playlist = playlist.into();

            let mut tracklist = TrackListValue::new(Some(playlist.tracks.clone()));

            tracklist.set_playlist(playlist);
            tracklist.set_list_type(TrackListType::Playlist);

            self.replace_list(tracklist.clone());

            let first_track = tracklist.queue.front_mut().unwrap();

            self.attach_track_url(first_track).await;
            self.set_current_track(first_track.clone());
            self.set_target_status(GstState::Playing);

            (Some(first_track.clone()), Some(tracklist))
        } else {
            (None, None)
        }
    }

    pub fn set_status(&mut self, status: StatusValue) {
        self.status = status;
    }

    pub fn status(&self) -> StatusValue {
        self.status.clone()
    }

    pub fn set_current_track(&mut self, track: Track) {
        self.current_track = Some(track);
    }

    pub fn set_position(&mut self, position: ClockValue) {
        self.position = position;
    }

    pub fn position(&self) -> ClockValue {
        self.position.clone()
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

    pub fn current_track_index(&self) -> Option<u8> {
        self.current_track.as_ref().map(|track| track.number)
    }

    pub fn replace_list(&mut self, tracklist: TrackListValue) {
        debug!("replacing tracklist");
        self.tracklist = tracklist;
    }

    pub fn track_list(&self) -> TrackListValue {
        self.tracklist.clone()
    }

    pub fn track_index(&self, track_id: usize) -> Option<u8> {
        if let Some(track) = self.tracklist.find_track(track_id) {
            Some(track.number)
        } else {
            None
        }
    }

    pub fn set_track_status(&mut self, track_id: usize, status: TrackStatus) {
        self.tracklist.set_track_status(track_id, status);
    }

    pub fn target_status(&self) -> StatusValue {
        self.target_status.clone()
    }

    pub fn set_target_status(&mut self, target: GstState) {
        self.target_status = target.into();
    }

    /// Attach a `TrackURL` to the given track.
    pub async fn attach_track_url(&mut self, track: &mut Track) {
        debug!("fetching track url");
        if let Ok(track_url) = self.client.track_url(track.id as i32, None, None).await {
            debug!("attaching url information to track");
            track.track_url = Some(track_url.url);
            track.sampling_rate = track_url.sampling_rate as f32;
            track.bit_depth = track_url.bit_depth as u8;
        }
    }

    pub async fn attach_track_url_current(&mut self) {
        if let Some(current_track) = self.current_track.as_mut() {
            if let Ok(track_url) = self
                .client
                .track_url(current_track.id as i32, None, None)
                .await
            {
                current_track.track_url = Some(track_url.url);
                current_track.bit_depth = track_url.bit_depth as u8;
                current_track.sampling_rate = track_url.sampling_rate as f32;
            }
        }
    }

    pub async fn next_track_url(&self) -> Option<String> {
        if let Some(current_index) = self.current_track_index() {
            if let Some(next_track) = self.tracklist.find_track_by_index(current_index + 1) {
                if let Ok(track_url) = self
                    .client
                    .track_url(next_track.id as i32, None, None)
                    .await
                {
                    return Some(track_url.url);
                } else {
                    return None;
                }
            }
        }

        None
    }

    pub async fn skip_track(
        &mut self,
        index: Option<u8>,
        direction: SkipDirection,
    ) -> Option<Track> {
        let next_track_index = if let Some(i) = index {
            if i <= self.tracklist.total() as u8 {
                Some(i)
            } else {
                None
            }
        } else if let Some(current_track_index) = self.current_track_index() {
            match direction {
                SkipDirection::Forward => {
                    if current_track_index < self.tracklist.total() as u8 {
                        Some(current_track_index + 1)
                    } else {
                        None
                    }
                }
                SkipDirection::Backward => {
                    if current_track_index > 1 {
                        Some(current_track_index - 1)
                    } else {
                        Some(0)
                    }
                }
            }
        } else {
            None
        };

        if let Some(index) = next_track_index {
            let mut current_track = None;

            for t in self.tracklist.queue.iter_mut() {
                match t.number.cmp(&index) {
                    std::cmp::Ordering::Less => {
                        t.status = TrackStatus::Played;
                    }
                    std::cmp::Ordering::Equal => {
                        t.status = TrackStatus::Playing;

                        if let Ok(track_url) = self.client.track_url(t.id as i32, None, None).await
                        {
                            t.track_url = Some(track_url.url);
                            t.bit_depth = track_url.bit_depth as u8;
                            t.sampling_rate = track_url.sampling_rate as f32;
                        }

                        self.current_track = Some(t.clone());
                        current_track = Some(t.clone());
                    }
                    std::cmp::Ordering::Greater => {
                        t.status = TrackStatus::Unplayed;
                    }
                }
            }

            current_track
        } else {
            debug!("no more tracks");
            None
        }
    }

    pub async fn search_all(&self, query: &str) -> Option<SearchAllResults> {
        match self.client.search_all(query.to_string(), 500).await {
            Ok(results) => Some(results),
            Err(_) => None,
        }
    }

    pub fn reset_player(&mut self) {
        self.target_status = GstState::Paused.into();
        self.position = ClockValue::default();
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
        self.position = ClockValue::default();
        self.status = gstreamer::State::Null.into();
        self.resume = false;
    }

    pub fn new(client: Client, db: Database) -> Self {
        let tracklist = TrackListValue::new(None);
        let (quit_sender, _) = tokio::sync::broadcast::channel::<bool>(1);

        Self {
            db,
            current_track: None,
            client,
            tracklist,
            position: ClockValue::default(),
            status: StatusValue(gstreamer::State::Null),
            target_status: StatusValue(gstreamer::State::Null),
            resume: false,
            quit_sender,
        }
    }

    pub async fn persist(&self) {
        debug!("persisting state to database");
        if self.current_track.is_some() {
            self.db.persist_state(self.clone()).await;
        }
    }

    pub async fn load_last_state(&mut self) -> bool {
        if let Some(last_state) = self.db.get_last_state().await {
            let entity_type: TrackListType = last_state.playback_entity_type.as_str().into();

            match entity_type {
                TrackListType::Album => {
                    if let Ok(album) = self.client.album(&last_state.playback_entity_id).await {
                        let album: Album = album.into();

                        self.replace_list(TrackListValue::new(Some(album.tracks.clone())));
                        self.tracklist.set_list_type(TrackListType::Album);
                        self.tracklist.set_album(album);

                        let position =
                            ClockTime::from_mseconds(last_state.playback_position as u64);

                        self.set_position(position.into());

                        self.skip_track(
                            Some(last_state.playback_track_index as u8),
                            SkipDirection::Forward,
                        )
                        .await;

                        return true;
                    }
                }
                TrackListType::Playlist => {
                    if let Ok(playlist) = self
                        .client
                        .playlist(
                            last_state
                                .playback_entity_id
                                .parse::<i64>()
                                .expect("failed to parse integer"),
                        )
                        .await
                    {
                        let playlist: Playlist = playlist.into();

                        self.replace_list(TrackListValue::new(Some(playlist.tracks.clone())));
                        self.tracklist.set_list_type(TrackListType::Playlist);
                        self.tracklist.set_playlist(playlist);

                        let position =
                            ClockTime::from_mseconds(last_state.playback_position as u64);
                        self.set_position(position.into());

                        self.skip_track(
                            Some(last_state.playback_track_index as u8),
                            SkipDirection::Forward,
                        )
                        .await;

                        return true;
                    }
                }
                TrackListType::Track => {
                    let track_id: i32 = last_state
                        .playback_entity_id
                        .parse()
                        .expect("failed to parse track id");
                    if let Ok(track) = self.client.track(track_id).await {
                        let mut track: Track = track.into();
                        track.status = TrackStatus::Playing;

                        let mut queue = VecDeque::new();
                        queue.push_front(track.clone());

                        let mut tracklist = TrackListValue::new(Some(queue));
                        tracklist.set_list_type(TrackListType::Track);

                        self.replace_list(tracklist);
                        self.tracklist.set_list_type(TrackListType::Track);

                        let position =
                            ClockTime::from_mseconds(last_state.playback_position as u64);

                        self.set_position(position.into());

                        self.skip_track(
                            Some(last_state.playback_track_index as u8),
                            SkipDirection::Forward,
                        )
                        .await;

                        return true;
                    }
                }
                TrackListType::Unknown => unreachable!(),
            }
        }

        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipDirection {
    Forward,
    Backward,
}

impl Display for SkipDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkipDirection::Forward => f.write_str("forward"),
            SkipDirection::Backward => f.write_str("backward"),
        }
    }
}
