use crate::{
    sql::db::Database,
    state::{ActiveScreen, ClockValue, FloatValue, StatusValue, TrackListType, TrackListValue},
    ui::components::{Row, TableRows},
};
use futures::executor;
use gstreamer::{ClockTime, State as GstState};
use hifirs_qobuz_api::client::{
    album::Album,
    api::Client,
    playlist::Playlist,
    track::{TrackListTrack, TrackStatus},
    AudioQuality,
};
use std::{
    collections::VecDeque,
    fmt::Display,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::sync::{
    broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender},
    RwLock,
};

#[derive(Debug, Clone)]
pub struct PlayerState {
    db: Database,
    client: Client,
    current_track: Option<TrackListTrack>,
    tracklist: TrackListValue,
    current_progress: FloatValue,
    duration: ClockValue,
    position: ClockValue,
    status: StatusValue,
    is_buffering: bool,
    resume: bool,
    target_status: StatusValue,
    active_screen: ActiveScreen,
    audio_quality: AudioQuality,
    quit_sender: BroadcastSender<bool>,
    jumps: usize,
    last_jump: SystemTime,
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
            let playback_track_id = current_track.track.id as i64;
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
                    .track
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
    pub fn set_active_screen(&mut self, screen: ActiveScreen) {
        self.active_screen = screen;
    }

    pub fn active_screen(&self) -> ActiveScreen {
        self.active_screen.clone()
    }

    pub fn set_status(&mut self, status: StatusValue) {
        self.status = status;
    }

    pub fn status(&self) -> StatusValue {
        self.status.clone()
    }

    pub fn set_current_track(&mut self, track: TrackListTrack) {
        self.current_track = Some(track);
    }

    pub fn set_position(&mut self, position: ClockValue) {
        self.position = position;
    }

    pub fn position(&self) -> ClockValue {
        self.position.clone()
    }

    pub fn set_buffering(&mut self, buffering: bool) {
        self.is_buffering = buffering;
    }

    pub fn buffering(&self) -> bool {
        self.is_buffering
    }

    pub fn set_resume(&mut self, resume: bool) {
        self.resume = resume;
    }

    pub fn resume(&self) -> bool {
        self.resume
    }

    pub fn progress(&self) -> FloatValue {
        let duration = self.duration.inner_clocktime().mseconds() as f64;
        let position = self.position.inner_clocktime().mseconds() as f64;

        if duration >= position {
            FloatValue(position / duration)
        } else {
            FloatValue(1.0)
        }
    }

    pub fn set_duration(&mut self, duration: ClockValue) {
        self.duration = duration;
    }

    pub fn duration(&self) -> ClockValue {
        self.duration.clone()
    }

    pub fn current_track(&self) -> Option<TrackListTrack> {
        self.current_track.clone()
    }

    pub fn unplayed_tracks(&self) -> Vec<&TrackListTrack> {
        self.tracklist.unplayed_tracks()
    }

    pub fn played_tracks(&self) -> Vec<&TrackListTrack> {
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

    pub fn current_track_index(&self) -> Option<usize> {
        self.current_track.as_ref().map(|track| track.index)
    }

    pub fn replace_list(&mut self, tracklist: TrackListValue) {
        debug!("replacing tracklist");
        self.tracklist = tracklist;
    }

    pub fn track_list(&self) -> TrackListValue {
        self.tracklist.clone()
    }

    pub fn track_index(&self, track_id: usize) -> Option<usize> {
        if let Some(track) = self.tracklist.find_track(track_id) {
            Some(track.index)
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

    pub fn rows(&self) -> Vec<Row> {
        self.tracklist.rows()
    }

    pub fn jumps(&self) -> usize {
        self.jumps
    }

    pub fn add_jump(&mut self) {
        self.jumps += 1;
    }

    pub fn sub_jump(&mut self) {
        self.jumps -= 1;
    }

    pub fn check_last_jump(&self) -> bool {
        self.last_jump
            .elapsed()
            .expect("failed to get elapsed time, should not occur")
            < Duration::from_millis(500)
    }

    /// Attach a `TrackURL` to the given track.
    pub async fn attach_track_url(&mut self, track: &mut TrackListTrack) {
        debug!("fetching track url");
        if let Ok(track_url) = self.client.track_url(track.track.id, None, None).await {
            debug!("attaching url information to track");
            track.set_track_url(track_url);
        }
    }

    pub async fn attach_track_url_current(&mut self) {
        if let Some(current_track) = self.current_track.as_mut() {
            if let Ok(track_url) = self
                .client
                .track_url(current_track.track.id, None, None)
                .await
            {
                current_track.set_track_url(track_url);
            }
        }
    }

    pub async fn next_track_url(&self) -> Option<String> {
        if let Some(current_index) = self.current_track_index() {
            if let Some(next_track) = self.tracklist.find_track_by_index(current_index + 1) {
                if let Ok(track_url) = self.client.track_url(next_track.track.id, None, None).await
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
        index: Option<usize>,
        direction: SkipDirection,
    ) -> Option<TrackListTrack> {
        let next_track_index = if let Some(i) = index {
            if i < self.tracklist.len() {
                Some(i)
            } else {
                None
            }
        } else if let Some(current_track_index) = self.current_track_index() {
            match direction {
                SkipDirection::Forward => {
                    if current_track_index < self.tracklist.len() - 1 {
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
            self.tracklist
                .queue
                .iter_mut()
                .for_each(|mut t| match t.index.cmp(&index) {
                    std::cmp::Ordering::Less => {
                        t.status = TrackStatus::Played;
                    }
                    std::cmp::Ordering::Equal => {
                        t.status = TrackStatus::Playing;
                        self.current_track = Some(t.clone());
                    }
                    std::cmp::Ordering::Greater => {
                        t.status = TrackStatus::Unplayed;
                    }
                });

            self.attach_track_url_current().await;
            self.current_track.clone()
        } else {
            debug!("no more tracks");
            None
        }
    }

    pub fn reset_player(&mut self) {
        self.target_status = GstState::Ready.into();
        self.position = ClockValue::default();
        self.current_progress = FloatValue(0.0);
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
        self.current_progress = FloatValue(0.0);
        self.duration = ClockValue::default();
        self.position = ClockValue::default();
        self.status = gstreamer::State::Null.into();
        self.is_buffering = false;
        self.resume = false;
    }

    pub fn new(client: Client, db: Database) -> Self {
        let tracklist = TrackListValue::new(None);
        let (quit_sender, _) = tokio::sync::broadcast::channel::<bool>(1);

        Self {
            db,
            current_track: None,
            audio_quality: client.quality(),
            client,
            tracklist,
            duration: ClockValue::default(),
            position: ClockValue::default(),
            status: StatusValue(gstreamer::State::Null),
            target_status: StatusValue(gstreamer::State::Null),
            current_progress: FloatValue(0.0),
            is_buffering: false,
            resume: false,
            active_screen: ActiveScreen::NowPlaying,
            quit_sender,
            jumps: 0,
            last_jump: SystemTime::now(),
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
                        self.replace_list(TrackListValue::new(
                            album.to_tracklist(self.audio_quality.clone()),
                        ));
                        self.tracklist.set_list_type(TrackListType::Album);
                        self.tracklist.set_album(album);

                        if let Some(track) = self
                            .tracklist
                            .find_track_by_index(last_state.playback_track_index as usize)
                        {
                            let duration = ClockTime::from_seconds(track.track.duration as u64);
                            let position =
                                ClockTime::from_mseconds(last_state.playback_position as u64);

                            self.set_position(position.into());
                            self.set_duration(duration.into());
                        }

                        self.skip_track(
                            Some(last_state.playback_track_index as usize),
                            SkipDirection::Forward,
                        )
                        .await;

                        return true;
                    }
                }
                TrackListType::Playlist => {
                    if let Ok(mut playlist) = self
                        .client
                        .playlist(
                            last_state
                                .playback_entity_id
                                .parse::<i64>()
                                .expect("failed to parse integer"),
                        )
                        .await
                    {
                        if let Some(tracklist_tracks) =
                            playlist.to_tracklist(self.audio_quality.clone())
                        {
                            self.replace_list(TrackListValue::new(Some(tracklist_tracks)));
                            self.tracklist.set_list_type(TrackListType::Playlist);
                            self.tracklist.set_playlist(playlist);

                            if let Some(track) = self
                                .tracklist
                                .find_track_by_index(last_state.playback_track_index as usize)
                            {
                                let duration = ClockTime::from_seconds(track.track.duration as u64);
                                let position =
                                    ClockTime::from_mseconds(last_state.playback_position as u64);

                                self.set_position(position.into());
                                self.set_duration(duration.into());
                            }

                            self.skip_track(
                                Some(last_state.playback_track_index as usize),
                                SkipDirection::Forward,
                            )
                            .await;

                            return true;
                        }
                    }
                }
                TrackListType::Track => {
                    let track_id: i32 = last_state
                        .playback_entity_id
                        .parse()
                        .expect("failed to parse track id");
                    if let Ok(track) = self.client.track(track_id).await {
                        let mut track = TrackListTrack::new(
                            track,
                            Some(0),
                            Some(1),
                            Some(self.audio_quality.clone()),
                            None,
                        );
                        track.status = TrackStatus::Playing;

                        let mut queue = VecDeque::new();
                        queue.push_front(track.clone());

                        let mut tracklist = TrackListValue::new(Some(queue));
                        tracklist.set_list_type(TrackListType::Track);

                        self.replace_list(tracklist);
                        self.tracklist.set_list_type(TrackListType::Track);

                        let duration = ClockTime::from_seconds(track.track.duration as u64);
                        let position =
                            ClockTime::from_mseconds(last_state.playback_position as u64);

                        self.set_position(position.into());
                        self.set_duration(duration.into());

                        self.skip_track(
                            Some(last_state.playback_track_index as usize),
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
