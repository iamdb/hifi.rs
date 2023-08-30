use crate::{cursive::CursiveFormat, qobuz::album::Album};
use cursive::{
    theme::{Effect, Style},
    utils::markup::StyledString,
};
use gstreamer::ClockTime;
use hifirs_qobuz_api::client::track::Track as QobuzTrack;
use serde::{Deserialize, Serialize};

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
    pub artist_name: Option<String>,
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
}

impl From<QobuzTrack> for Track {
    fn from(value: QobuzTrack) -> Self {
        let album = if let Some(album) = &value.album {
            let a: Album = album.clone().into();
            Some(a)
        } else {
            None
        };

        let artist_name = if let Some(p) = &value.performer {
            Some(p.name.clone())
        } else {
            value.album.as_ref().map(|a| a.artist.name.clone())
        };

        let cover_art = value.album.as_ref().map(|a| a.image.large.clone());

        let status = if value.streamable {
            TrackStatus::Unplayed
        } else {
            TrackStatus::Unplayable
        };

        Self {
            id: value.id as usize,
            number: value.track_number as usize,
            title: value.title,
            album,
            artist_name,
            duration_seconds: value.duration as usize,
            explicit: value.parental_warning,
            hires_available: value.hires_streamable,
            sampling_rate: 0.,
            bit_depth: 0,
            status,
            track_url: None,
            available: value.streamable,
            cover_art,
        }
    }
}

impl CursiveFormat for Track {
    fn list_item(&self) -> StyledString {
        let mut style = Style::none();

        if !self.available {
            style = style.combine(Effect::Dim).combine(Effect::Strikethrough);
        }

        let mut title = StyledString::styled(self.title.trim(), style.combine(Effect::Bold));

        if let Some(artist) = &self.artist_name {
            title.append_styled(" by ", style);
            title.append_styled(artist, style);
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
    fn track_list_item(&self, inactive: bool, index: Option<usize>) -> StyledString {
        let mut style = Style::none();

        if inactive || !self.available {
            style = style
                .combine(Effect::Dim)
                .combine(Effect::Italic)
                .combine(Effect::Strikethrough);
        }

        let num = if let Some(index) = index {
            index + 1
        } else {
            self.number
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
