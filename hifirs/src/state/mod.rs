pub mod app;

use crate::{
    state::app::StateKey,
    ui::components::{Item, Row, TableRow, TableRows},
};
use clap::ValueEnum;
use gst::{ClockTime, State as GstState};
use gstreamer as gst;
use qobuz_client::client::{album::Album, playlist::Playlist, track::PlaylistTrack, AudioQuality};
use serde::{Deserialize, Serialize};
use sled::{IVec, Tree};
use std::{
    collections::{vec_deque::Drain, VecDeque},
    fmt::Display,
    ops::RangeBounds,
    str::FromStr,
};
use tui::{
    style::{Color, Modifier, Style},
    text::Text,
    widgets::ListItem,
};

#[derive(Debug, Clone)]
pub struct HifiDB(sled::Db);

impl HifiDB {
    pub fn open_tree(&self, name: &'static str) -> StateTree {
        StateTree::new(
            self.0
                .open_tree(name)
                .unwrap_or_else(|_| panic!("failed to open tree {}", name)),
        )
    }
}

#[derive(Debug, Clone)]
pub struct StateTree {
    db: Tree,
}

impl StateTree {
    pub fn new(db: Tree) -> StateTree {
        StateTree { db }
    }
    pub fn clear(&self) {
        self.db.clear().expect("failed to clear tree");
    }
    pub fn flush(&self) {
        self.db.flush().expect("failed to flush db");
    }
    pub fn insert<K, T>(&self, key: StateKey, value: T)
    where
        K: FromStr,
        T: Serialize,
    {
        if let Ok(serialized) = bincode::serialize(&value) {
            self.db.insert(key.as_str(), serialized).unwrap();
        }
    }
    pub fn get<'a, K, T>(&self, key: StateKey) -> Option<T>
    where
        K: FromStr,
        T: Into<T> + From<Bytes> + Deserialize<'a>,
    {
        if let Ok(record) = self.db.get(key.as_str()) {
            record.map(|value| {
                let bytes: Bytes = value.into();

                bytes.into()
            })
        } else {
            None
        }
    }
}

#[macro_export]
macro_rules! get_client {
    ($tree_key:path, $tree:ident, $value_type:ty) => {
        $tree.get::<String, $value_type>(StateKey::Client($tree_key))
    };
}

#[macro_export]
macro_rules! get_player {
    ($tree_key:path, $tree:ident, $value_type:ty) => {
        $tree.get::<String, $value_type>(StateKey::Player($tree_key))
    };
}

#[macro_export]
macro_rules! get_app {
    ($tree_key:path, $tree:ident, $value_type:ty) => {
        $tree.get::<String, $value_type>(StateKey::App($tree_key))
    };
}

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

impl From<IVec> for Bytes {
    fn from(ivec: IVec) -> Self {
        ivec.to_vec().into()
    }
}

impl From<Bytes> for IVec {
    fn from(bytes: Bytes) -> Self {
        bytes.into()
    }
}

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

impl From<Bytes> for PlaylistTrack {
    fn from(bytes: Bytes) -> Self {
        let deserialized: PlaylistTrack =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize playlist track");

        deserialized
    }
}

/// A playlist is a list of tracks.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TrackListValue {
    queue: VecDeque<PlaylistTrack>,
    album: Option<Album>,
    playlist: Option<Playlist>,
}

impl From<Bytes> for TrackListValue {
    fn from(bytes: Bytes) -> Self {
        let deserialized: TrackListValue =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize status value");

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
        self.queue
            .iter()
            .map(|i| i.track.row())
            .collect::<Vec<Row>>()
    }
}

impl TrackListValue {
    pub fn new() -> TrackListValue {
        TrackListValue {
            queue: VecDeque::new(),
            album: None,
            playlist: None,
        }
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }

    pub fn set_album(&mut self, album: Album) {
        self.album = Some(album);
    }

    pub fn get_album(&self) -> Option<&Album> {
        self.album.as_ref()
    }

    pub fn set_playlist(&mut self, playlist: Playlist) {
        self.playlist = Some(playlist);
    }

    pub fn get_playlist(&self) -> Option<&Playlist> {
        self.playlist.as_ref()
    }

    pub fn find_track(&self, track_id: usize) -> Option<PlaylistTrack> {
        self.queue
            .iter()
            .find(|t| t.track.id as usize == track_id)
            .cloned()
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

    pub fn vec(&self) -> VecDeque<PlaylistTrack> {
        self.queue.clone()
    }

    pub fn drain<R>(&mut self, range: R) -> Drain<PlaylistTrack>
    where
        R: RangeBounds<usize>,
    {
        self.queue.drain(range)
    }

    pub fn append(&mut self, mut items: VecDeque<PlaylistTrack>) {
        self.queue.append(&mut items)
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn front(&self) -> Option<&PlaylistTrack> {
        self.queue.front()
    }

    pub fn back(&self) -> Option<&PlaylistTrack> {
        self.queue.back()
    }

    pub fn pop_front(&mut self) -> Option<PlaylistTrack> {
        self.queue.pop_front()
    }

    pub fn pop_back(&mut self) -> Option<PlaylistTrack> {
        self.queue.pop_back()
    }

    pub fn push_front(&mut self, track: PlaylistTrack) {
        self.queue.push_front(track);
    }

    pub fn push_back(&mut self, track: PlaylistTrack) {
        self.queue.push_back(track);
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
