use crate::{cursive::CursiveFormat, state::TrackListType};
use async_trait::async_trait;
use cursive::{
    theme::{Effect, Style},
    utils::markup::StyledString,
};
use gstreamer::ClockTime;
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, fmt::Debug};

#[async_trait]
pub trait MusicService: Send + Sync + Debug {
    async fn login(&self);
    async fn album(&self, album_id: &str) -> Option<Album>;
    async fn track(&self, track_id: i32) -> Option<Track>;
    async fn artist(&self, artist_id: i32) -> Option<Artist>;
    async fn playlist(&self, playlist_id: i64) -> Option<Playlist>;
    async fn search(&self, query: &str) -> Option<SearchResults>;
    async fn track_url(&self, track_id: i32) -> Option<String>;
    async fn user_playlists(&self) -> Option<Vec<Playlist>>;
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrackStatus {
    Played,
    Playing,
    #[default]
    Unplayed,
    Unplayable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: usize,
    pub number: usize,
    pub title: String,
    pub album: Option<Album>,
    pub artist: Option<Artist>,
    pub duration_seconds: usize,
    pub explicit: bool,
    pub hires_available: bool,
    pub sampling_rate: f32,
    pub bit_depth: usize,
    pub status: TrackStatus,
    #[serde(skip)]
    pub track_url: Option<String>,
    pub available: bool,
    pub cover_art: Option<String>,
    pub position: usize,
    pub media_number: usize,
}

impl CursiveFormat for Track {
    fn list_item(&self) -> StyledString {
        let mut style = Style::none();

        if !self.available {
            style = style.combine(Effect::Dim).combine(Effect::Strikethrough);
        }

        let mut title = StyledString::styled(self.title.trim(), style.combine(Effect::Bold));

        if let Some(artist) = &self.artist {
            title.append_styled(" by ", style);
            title.append_styled(&artist.name, style);
        }

        let duration = ClockTime::from_seconds(self.duration_seconds as u64)
            .to_string()
            .as_str()[2..7]
            .to_string();
        title.append_plain(" ");
        title.append_styled(duration, style.combine(Effect::Dim));
        title.append_plain(" ");

        if self.explicit {
            title.append_styled("e", style.combine(Effect::Dim));
        }

        if self.hires_available {
            title.append_styled("*", style.combine(Effect::Dim));
        }

        title
    }
    fn track_list_item(&self, list_type: &TrackListType, inactive: bool) -> StyledString {
        let mut style = Style::none();

        if inactive || !self.available {
            style = style
                .combine(Effect::Dim)
                .combine(Effect::Italic)
                .combine(Effect::Strikethrough);
        }

        let num = match list_type {
            TrackListType::Album => self.number,
            TrackListType::Playlist => self.position,
            TrackListType::Track => self.number,
            TrackListType::Unknown => self.position,
        };

        let mut item = StyledString::styled(format!("{:02} ", num), style);
        item.append_styled(self.title.trim(), style.combine(Effect::Simple));
        item.append_plain(" ");

        let duration = ClockTime::from_seconds(self.duration_seconds as u64)
            .to_string()
            .as_str()[2..7]
            .to_string();

        item.append_styled(duration, style.combine(Effect::Dim));

        item
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: Artist,
    pub release_year: usize,
    pub hires_available: bool,
    pub explicit: bool,
    pub total_tracks: usize,
    pub tracks: VecDeque<Track>,
    pub available: bool,
    pub cover_art: String,
}

impl CursiveFormat for Album {
    fn list_item(&self) -> StyledString {
        let mut style = Style::none();

        if !self.available {
            style = style.combine(Effect::Dim).combine(Effect::Strikethrough);
        }

        let mut title = StyledString::styled(self.title.clone(), style.combine(Effect::Bold));

        title.append_styled(" by ", style);
        title.append_styled(self.artist.name.clone(), style);
        title.append_styled(" ", style);

        title.append_styled(self.release_year.to_string(), style.combine(Effect::Dim));
        title.append_plain(" ");

        if self.explicit {
            title.append_styled("e", style.combine(Effect::Dim));
        }

        if self.hires_available {
            title.append_styled("*", style.combine(Effect::Dim));
        }

        title
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub query: String,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
    pub artists: Vec<Artist>,
    pub playlists: Vec<Playlist>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: usize,
    pub name: String,
    pub albums: Option<Vec<Album>>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub title: String,
    pub duration_seconds: usize,
    pub tracks_count: usize,
    pub id: usize,
    pub cover_art: Option<String>,
    pub tracks: VecDeque<Track>,
}

impl CursiveFormat for Artist {
    fn list_item(&self) -> StyledString {
        StyledString::plain(self.name.clone())
    }
}
