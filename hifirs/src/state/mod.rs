pub mod app;

use crate::ui::components::{Item, Row, TableRow, TableRows};
use gst::{ClockTime, State as GstState};
use gstreamer as gst;
use qobuz_client::client::{
    album::Album,
    playlist::Playlist,
    track::{TrackListTrack, TrackStatus},
    AudioQuality,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{vec_deque::Drain, VecDeque},
    fmt::Display,
    ops::RangeBounds,
};
use tui::{
    style::{Color, Modifier, Style},
    text::Text,
    widgets::ListItem,
};

#[derive(Debug, Eq, PartialEq, Hash, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ActiveScreen {
    NowPlaying,
    Search,
    Playlists,
}

impl ActiveScreen {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActiveScreen::NowPlaying => "now_playing",
            ActiveScreen::Search => "search",
            ActiveScreen::Playlists => "playlists",
        }
    }
}

impl From<Bytes> for ActiveScreen {
    fn from(bytes: Bytes) -> Self {
        let deserialized: ActiveScreen =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize status value");

        deserialized
    }
}

impl From<ActiveScreen> for Bytes {
    fn from(screen: ActiveScreen) -> Self {
        bincode::serialize(&screen)
            .expect("failed to serialize string value")
            .into()
    }
}

/// A wrapper for string values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringValue(String);

impl From<Bytes> for StringValue {
    fn from(bytes: Bytes) -> Self {
        let deserialized: StringValue =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize status value");

        deserialized
    }
}

impl From<StringValue> for Bytes {
    fn from(string_value: StringValue) -> Self {
        bincode::serialize(&string_value)
            .expect("failed to serialize string value")
            .into()
    }
}

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

impl From<Bytes> for StatusValue {
    fn from(bytes: Bytes) -> Self {
        let deserialized: StatusValue =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize status value");

        deserialized
    }
}

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
    pub fn as_str(&self) -> &'static str {
        match self.0 {
            GstState::Playing => "Playing",
            GstState::Paused => "Paused",
            GstState::Null => "Stopped",
            GstState::VoidPending => "Stopped",
            GstState::Ready => "Stopped",
            _ => "",
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

impl From<ClockValue> for Bytes {
    fn from(clock_value: ClockValue) -> Self {
        bincode::serialize(&clock_value)
            .expect("failed to serialize clock value")
            .into()
    }
}

impl From<Bytes> for ClockValue {
    fn from(bytes: Bytes) -> Self {
        bincode::deserialize(&bytes.vec()).expect("failed to deserialize vec<u8>")
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

impl From<Bytes> for FloatValue {
    fn from(bytes: Bytes) -> Self {
        let deserialized: FloatValue =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize float value");

        deserialized
    }
}

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

#[derive(Debug, Clone)]
pub struct Bytes(Vec<u8>);

impl From<Vec<u8>> for Bytes {
    fn from(vec: Vec<u8>) -> Self {
        Bytes(vec)
    }
}

impl From<Bytes> for Vec<u8> {
    fn from(bytes: Bytes) -> Self {
        bytes.0
    }
}

impl Bytes {
    pub fn vec(&self) -> Vec<u8> {
        self.0.clone()
    }
}

impl From<Bytes> for AudioQuality {
    fn from(bytes: Bytes) -> Self {
        let deserialized: AudioQuality =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize audio quality");

        deserialized
    }
}

impl From<AudioQuality> for Bytes {
    fn from(audio_quality: AudioQuality) -> Self {
        bincode::serialize(&audio_quality)
            .expect("failed to serialize audio quality")
            .into()
    }
}

impl From<Bytes> for TrackListTrack {
    fn from(bytes: Bytes) -> Self {
        let deserialized: TrackListTrack =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize playlist track");

        deserialized
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrackListType {
    #[default]
    Album,
    Playlist,
}

/// A tracklist is a list of tracks.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TrackListValue {
    queue: VecDeque<TrackListTrack>,
    album: Option<Album>,
    playlist: Option<Playlist>,
    list_type: Option<TrackListType>,
}

impl From<Bytes> for TrackListValue {
    fn from(bytes: Bytes) -> Self {
        let deserialized: TrackListValue =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize tracklist value");

        deserialized
    }
}

impl From<TrackListValue> for Bytes {
    fn from(playlist: TrackListValue) -> Self {
        bincode::serialize(&playlist)
            .expect("failed to serialize playlist")
            .into()
    }
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
    pub fn new(queue: Option<VecDeque<TrackListTrack>>) -> TrackListValue {
        debug!("creating tracklist");
        let queue = if let Some(q) = queue {
            q
        } else {
            VecDeque::new()
        };

        TrackListValue {
            queue,
            album: None,
            playlist: None,
            list_type: None,
        }
    }

    pub fn clear(&mut self) {
        self.list_type = None;
        self.album = None;
        self.playlist = None;
        self.queue.clear();
    }

    pub fn set_album(&mut self, album: Album) {
        debug!("setting tracklist album");
        self.album = Some(album);
        debug!("setting tracklist list type");
        self.list_type = Some(TrackListType::Album);
    }

    pub fn get_album(&self) -> Option<&Album> {
        self.album.as_ref()
    }

    pub fn set_playlist(&mut self, playlist: Playlist) {
        self.playlist = Some(playlist);
        self.list_type = Some(TrackListType::Playlist);
    }

    pub fn get_playlist(&self) -> Option<&Playlist> {
        self.playlist.as_ref()
    }

    pub fn set_list_type(&mut self, list_type: TrackListType) {
        self.list_type = Some(list_type);
    }

    pub fn list_type(&self) -> Option<&TrackListType> {
        self.list_type.as_ref()
    }

    pub fn find_track(&self, track_id: usize) -> Option<TrackListTrack> {
        self.queue
            .iter()
            .find(|t| t.track.id as usize == track_id)
            .cloned()
    }

    pub fn find_track_by_index(&self, index: usize) -> Option<TrackListTrack> {
        self.queue.iter().find(|t| t.index == index).cloned()
    }

    pub fn set_track_status(&mut self, track_id: usize, status: TrackStatus) {
        if let Some(track) = self
            .queue
            .iter_mut()
            .find(|t| t.track.id as usize == track_id)
        {
            track.status = status;
        }
    }

    pub fn unplayed_tracks(&self) -> Vec<&TrackListTrack> {
        self.queue
            .iter()
            .filter(|t| t.status == TrackStatus::Unplayed)
            .collect::<Vec<&TrackListTrack>>()
    }

    pub fn played_tracks(&self) -> Vec<&TrackListTrack> {
        self.queue
            .iter()
            .filter(|t| t.status == TrackStatus::Played)
            .collect::<Vec<&TrackListTrack>>()
    }

    pub fn track_index(&self, track_id: usize) -> Option<usize> {
        let mut index: Option<usize> = None;

        self.queue.iter().enumerate().for_each(|(i, t)| {
            if t.track.id as usize == track_id {
                index = Some(i);
            }
        });

        index
    }

    pub fn item_list(self, max_width: usize, dim: bool) -> Vec<Item<'static>> {
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
}
