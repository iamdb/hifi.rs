pub mod app;

use crate::ui::components::{Item, Row, TableRow, TableRows};
use gst::{ClockTime, State as GstState};
use gstreamer as gst;
use hifirs_qobuz_api::client::{
    album::Album,
    playlist::Playlist,
    track::{TrackListTrack, TrackStatus},
};
use ratatui::{
    style::{Color, Modifier, Style},
    text::Text,
    widgets::ListItem,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{vec_deque::Drain, VecDeque},
    fmt::Display,
    ops::RangeBounds,
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
    queue: VecDeque<TrackListTrack>,
    album: Option<Album>,
    playlist: Option<Playlist>,
    list_type: TrackListType,
}

impl TableRows for TrackListValue {
    fn rows(&self) -> Vec<Row> {
        let mut rows = self.unplayed_tracks();
        rows.append(&mut self.played_tracks());

        rows.iter()
            .filter_map(|t| {
                if t.status != TrackStatus::Playing {
                    Some(t.row())
                } else {
                    None
                }
            })
            .collect::<Vec<Row>>()
    }
}

impl TrackListValue {
    #[instrument]
    pub fn new(queue: Option<VecDeque<TrackListTrack>>) -> TrackListValue {
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

    #[instrument]
    pub fn clear(&mut self) {
        self.list_type = TrackListType::Unknown;
        self.album = None;
        self.playlist = None;
        self.queue.clear();
    }

    #[instrument]
    pub fn set_album(&mut self, album: Album) {
        debug!("setting tracklist album");
        self.album = Some(album);
        debug!("setting tracklist list type");
        self.list_type = TrackListType::Album;
    }

    #[instrument]
    pub fn get_album(&self) -> Option<&Album> {
        self.album.as_ref()
    }

    #[instrument]
    pub fn set_playlist(&mut self, playlist: Playlist) {
        self.playlist = Some(playlist);
        self.list_type = TrackListType::Playlist;
    }

    #[instrument]
    pub fn get_playlist(&self) -> Option<&Playlist> {
        self.playlist.as_ref()
    }

    #[instrument]
    pub fn set_list_type(&mut self, list_type: TrackListType) {
        self.list_type = list_type;
    }

    #[instrument]
    pub fn list_type(&self) -> &TrackListType {
        &self.list_type
    }

    #[instrument]
    pub fn find_track(&self, track_id: usize) -> Option<TrackListTrack> {
        self.queue
            .iter()
            .find(|t| t.track.id as usize == track_id)
            .cloned()
    }

    #[instrument]
    pub fn find_track_by_index(&self, index: usize) -> Option<TrackListTrack> {
        self.queue.iter().find(|t| t.index == index).cloned()
    }

    #[instrument]
    pub fn set_track_status(&mut self, track_id: usize, status: TrackStatus) {
        if let Some(track) = self
            .queue
            .iter_mut()
            .find(|t| t.track.id as usize == track_id)
        {
            track.status = status;
        }
    }

    #[instrument]
    pub fn unplayed_tracks(&self) -> Vec<&TrackListTrack> {
        self.queue
            .iter()
            .filter(|t| t.status == TrackStatus::Unplayed)
            .collect::<Vec<&TrackListTrack>>()
    }

    #[instrument]
    pub fn played_tracks(&self) -> Vec<&TrackListTrack> {
        self.queue
            .iter()
            .filter(|t| t.status == TrackStatus::Played)
            .collect::<Vec<&TrackListTrack>>()
    }

    #[instrument]
    pub fn track_index(&self, track_id: usize) -> Option<usize> {
        let mut index: Option<usize> = None;

        self.queue.iter().enumerate().for_each(|(i, t)| {
            if t.track.id as usize == track_id {
                index = Some(i);
            }
        });

        index
    }

    #[instrument]
    pub fn item_list<'a>(self, max_width: usize, dim: bool) -> Vec<Item<'a>> {
        self.queue
            .into_iter()
            .map(|t| {
                let title = textwrap::wrap(
                    format!("{:02} {}", t.track.track_number, t.track.title).as_str(),
                    max_width,
                )
                .join("\n   ");

                let mut style = Style::default().fg(Color::White);

                if dim {
                    style = style.add_modifier(Modifier::DIM);
                }

                ListItem::new(Text::raw(title)).style(style).into()
            })
            .collect::<Vec<Item>>()
    }

    pub fn vec(&self) -> VecDeque<TrackListTrack> {
        self.queue.clone()
    }

    pub fn drain<R>(&mut self, range: R) -> Drain<TrackListTrack>
    where
        R: RangeBounds<usize>,
    {
        self.queue.drain(range)
    }

    pub fn append(&mut self, mut items: VecDeque<TrackListTrack>) {
        self.queue.append(&mut items)
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn front(&self) -> Option<&TrackListTrack> {
        self.queue.front()
    }

    pub fn back(&self) -> Option<&TrackListTrack> {
        self.queue.back()
    }

    pub fn pop_front(&mut self) -> Option<TrackListTrack> {
        self.queue.pop_front()
    }

    pub fn pop_back(&mut self) -> Option<TrackListTrack> {
        self.queue.pop_back()
    }

    pub fn push_front(&mut self, track: TrackListTrack) {
        self.queue.push_front(track);
    }

    pub fn push_back(&mut self, track: TrackListTrack) {
        self.queue.push_back(track);
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn cursive_list(&self) -> Vec<(String, i32)> {
        self.queue
            .iter()
            .map(|i| (i.track.title.clone(), i.track.id))
            .collect::<Vec<(String, i32)>>()
    }
}
