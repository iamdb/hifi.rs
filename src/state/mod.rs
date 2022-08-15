pub mod app;

use crate::qobuz::PlaylistTrack;
use crate::state::app::{PlayerKey, StateKey};
use crate::ui::terminal::components::Item;

use clap::ValueEnum;
use gst::{ClockTime, State as GstState};
use gstreamer as gst;
use serde::{Deserialize, Serialize};
use sled::{IVec, Tree};
use std::collections::vec_deque::Drain;
use std::collections::VecDeque;
use std::fmt::Display;
use std::ops::RangeBounds;
use std::str::FromStr;
use tui::style::{Color, Modifier, Style};
use tui::text::Text;
use tui::widgets::ListItem;

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
    pub fn item_list(&self, max_width: usize) -> Option<Vec<Item<'static>>> {
        if let Some(playlist) = crate::get_player!(PlayerKey::Playlist, self, PlaylistValue) {
            let mut items = playlist.item_list(max_width, false);

            if let Some(prev_playlist) =
                crate::get_player!(PlayerKey::PreviousPlaylist, self, PlaylistValue)
            {
                let mut prev_items = prev_playlist.item_list(max_width, true);

                items.append(&mut prev_items);
            }

            Some(items)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ActiveScreen {
    NowPlaying,
    Search,
}

impl ActiveScreen {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActiveScreen::NowPlaying => "now_playing",
            ActiveScreen::Search => "search",
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

/// The audio quality as defined by the Qobuz API.
#[derive(Clone, Debug, Serialize, Deserialize, ValueEnum)]
pub enum AudioQuality {
    Mp3 = 5,
    CD = 6,
    HIFI96 = 7,
    HIFI192 = 27,
}

impl Display for AudioQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.clone() as u32))
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

/// A playlist is a list of tracks.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistValue(VecDeque<PlaylistTrack>);

impl From<Bytes> for PlaylistValue {
    fn from(bytes: Bytes) -> Self {
        let deserialized: PlaylistValue =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize status value");

        deserialized
    }
}

impl From<PlaylistValue> for Bytes {
    fn from(playlist: PlaylistValue) -> Self {
        bincode::serialize(&playlist)
            .expect("failed to serialize playlist")
            .into()
    }
}

impl IntoIterator for PlaylistValue {
    type Item = PlaylistTrack;

    type IntoIter = std::collections::vec_deque::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl PlaylistValue {
    pub fn new() -> PlaylistValue {
        PlaylistValue(VecDeque::new())
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn item_list(self, max_width: usize, dim: bool) -> Vec<Item<'static>> {
        self.into_iter()
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
        self.0.clone()
    }

    pub fn drain<R>(&mut self, range: R) -> Drain<PlaylistTrack>
    where
        R: RangeBounds<usize>,
    {
        self.0.drain(range)
    }

    pub fn append(&mut self, mut items: VecDeque<PlaylistTrack>) {
        self.0.append(&mut items)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn front(&self) -> Option<&PlaylistTrack> {
        self.0.front()
    }

    pub fn back(&self) -> Option<&PlaylistTrack> {
        self.0.back()
    }

    pub fn pop_front(&mut self) -> Option<PlaylistTrack> {
        self.0.pop_front()
    }

    pub fn pop_back(&mut self) -> Option<PlaylistTrack> {
        self.0.pop_back()
    }

    pub fn push_front(&mut self, track: PlaylistTrack) {
        self.0.push_front(track)
    }

    pub fn push_back(&mut self, track: PlaylistTrack) {
        self.0.push_back(track)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
