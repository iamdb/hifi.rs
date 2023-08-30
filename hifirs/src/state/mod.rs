pub mod app;

use gst::{ClockTime, State as GstState};
use gstreamer as gst;
use serde::{Deserialize, Serialize};
use std::{
    collections::{vec_deque::Drain, VecDeque},
    fmt::Display,
    ops::RangeBounds,
};

use crate::qobuz::{
    album::Album,
    playlist::Playlist,
    track::{Track, TrackStatus},
};

#[derive(Debug, Eq, PartialEq, Hash, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ActiveScreen {
    NowPlaying,
    Search,
    Playlists,
}

impl ActiveScreen {
    pub fn as_str(&self) -> &str {
        match self {
            ActiveScreen::NowPlaying => "now_playing",
            ActiveScreen::Search => "search",
            ActiveScreen::Playlists => "playlists",
        }
    }
}

/// A wrapper for string values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringValue(String);

impl From<String> for StringValue {
    fn from(string: String) -> Self {
        StringValue(string)
    }
}

impl Display for StringValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl StringValue {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// A wrapper for gstreamer state values
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct StatusValue(GstState);

impl From<GstState> for StatusValue {
    fn from(state: GstState) -> Self {
        StatusValue(state)
    }
}

impl From<StatusValue> for GstState {
    fn from(state: StatusValue) -> Self {
        state.0
    }
}

impl StatusValue {
    pub fn as_str(&self) -> &str {
        match self.0 {
            GstState::Playing => "Playing",
            GstState::Paused => "Paused",
            GstState::Null => "Stopped",
            GstState::VoidPending => "Stopped",
            GstState::Ready => "Ready",
        }
    }
}

/// A wrapper for ClockTime values
#[derive(Default, Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Deserialize)]
pub struct ClockValue(ClockTime);

impl ClockValue {
    /// Retreive the ClockTime value wrapped by this type.
    pub fn inner_clocktime(&self) -> ClockTime {
        self.0
    }
}

impl From<ClockTime> for ClockValue {
    fn from(time: ClockTime) -> Self {
        ClockValue(time)
    }
}

impl From<ClockValue> for ClockTime {
    fn from(clock_value: ClockValue) -> Self {
        clock_value.0
    }
}

impl Display for ClockValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.to_string().as_str())
    }
}

/// A wrapper for float values
#[derive(Debug, Clone, Serialize, PartialEq, PartialOrd, Deserialize)]
pub struct FloatValue(pub f64);

impl From<f64> for FloatValue {
    fn from(float: f64) -> Self {
        FloatValue(float)
    }
}

impl From<FloatValue> for f64 {
    fn from(float: FloatValue) -> Self {
        float.0
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrackListType {
    Album,
    Playlist,
    Track,
    #[default]
    Unknown,
}

impl Display for TrackListType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackListType::Album => f.write_fmt(format_args!("album")),
            TrackListType::Playlist => f.write_fmt(format_args!("playlist")),
            TrackListType::Track => f.write_fmt(format_args!("track")),
            TrackListType::Unknown => f.write_fmt(format_args!("unknown")),
        }
    }
}

impl From<&str> for TrackListType {
    fn from(tracklist_type: &str) -> Self {
        match tracklist_type {
            "album" => TrackListType::Album,
            "playlist" => TrackListType::Playlist,
            "track" => TrackListType::Track,
            _ => TrackListType::Unknown,
        }
    }
}

/// A tracklist is a list of tracks.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TrackListValue {
    queue: VecDeque<Track>,
    album: Option<Album>,
    playlist: Option<Playlist>,
    list_type: TrackListType,
}

impl TrackListValue {
    #[instrument]
    pub fn new(queue: Option<VecDeque<Track>>) -> TrackListValue {
        let queue = if let Some(q) = queue {
            q
        } else {
            VecDeque::new()
        };

        TrackListValue {
            queue,
            album: None,
            playlist: None,
            list_type: TrackListType::Unknown,
        }
    }

    pub fn total(&self) -> usize {
        if let Some(album) = &self.album {
            album.total_tracks
        } else if let Some(list) = &self.playlist {
            list.tracks_count
        } else {
            self.queue.len()
        }
    }

    #[instrument(skip(self))]
    pub fn clear(&mut self) {
        self.list_type = TrackListType::Unknown;
        self.album = None;
        self.playlist = None;
        self.queue.clear();
    }

    #[instrument(skip(self))]
    pub fn set_album(&mut self, album: Album) {
        debug!("setting tracklist album");
        self.album = Some(album);
        debug!("setting tracklist list type");
        self.list_type = TrackListType::Album;
    }

    #[instrument(skip(self))]
    pub fn get_album(&self) -> Option<&Album> {
        self.album.as_ref()
    }

    #[instrument(skip(self))]
    pub fn set_playlist(&mut self, playlist: Playlist) {
        self.playlist = Some(playlist);
        self.list_type = TrackListType::Playlist;
    }

    #[instrument(skip(self))]
    pub fn get_playlist(&self) -> Option<&Playlist> {
        self.playlist.as_ref()
    }

    #[instrument(skip(self))]
    pub fn set_list_type(&mut self, list_type: TrackListType) {
        self.list_type = list_type;
    }

    #[instrument(skip(self))]
    pub fn list_type(&self) -> &TrackListType {
        &self.list_type
    }

    #[instrument(skip(self))]
    pub fn find_track(&self, track_id: usize) -> Option<Track> {
        self.queue.iter().find(|t| t.id == track_id).cloned()
    }

    #[instrument(skip(self))]
    pub fn find_track_by_index(&self, index: usize) -> Option<Track> {
        self.queue.iter().find(|t| t.position == index).cloned()
    }

    #[instrument(skip(self))]
    pub fn set_track_status(&mut self, position: usize, status: TrackStatus) {
        if let Some(track) = self.queue.iter_mut().find(|t| t.position == position) {
            track.status = status;
        }
    }

    #[instrument(skip(self))]
    pub fn unplayed_tracks(&self) -> Vec<&Track> {
        self.queue
            .iter()
            .filter(|t| t.status == TrackStatus::Unplayed)
            .collect::<Vec<&Track>>()
    }

    #[instrument(skip(self))]
    pub fn played_tracks(&self) -> Vec<&Track> {
        self.queue
            .iter()
            .filter(|t| t.status == TrackStatus::Played)
            .collect::<Vec<&Track>>()
    }

    #[instrument(skip(self))]
    pub fn track_index(&self, track_id: usize) -> Option<usize> {
        let mut index: Option<usize> = None;

        self.queue.iter().enumerate().for_each(|(i, t)| {
            if t.id == track_id {
                index = Some(i);
            }
        });

        index
    }

    pub fn current_track(&self) -> Option<Track> {
        for track in &self.queue {
            if track.status == TrackStatus::Playing {
                return Some(track.clone());
            }
        }

        None
    }

    #[instrument(skip(self))]
    pub fn vec(&self) -> VecDeque<Track> {
        self.queue.clone()
    }

    pub fn drain<R>(&mut self, range: R) -> Drain<Track>
    where
        R: RangeBounds<usize>,
    {
        self.queue.drain(range)
    }

    pub fn append(&mut self, mut items: VecDeque<Track>) {
        self.queue.append(&mut items)
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn front(&self) -> Option<&Track> {
        self.queue.front()
    }

    pub fn back(&self) -> Option<&Track> {
        self.queue.back()
    }

    pub fn pop_front(&mut self) -> Option<Track> {
        self.queue.pop_front()
    }

    pub fn pop_back(&mut self) -> Option<Track> {
        self.queue.pop_back()
    }

    pub fn push_front(&mut self, track: Track) {
        self.queue.push_front(track);
    }

    pub fn push_back(&mut self, track: Track) {
        self.queue.push_back(track);
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn cursive_list(&self) -> Vec<(String, i32)> {
        self.queue
            .iter()
            .map(|i| (i.title.clone(), i.id as i32))
            .collect::<Vec<(String, i32)>>()
    }
}
