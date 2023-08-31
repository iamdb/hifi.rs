use crate::{
    cursive::CursiveFormat,
    qobuz::{track::Track, Artist},
};
use cursive::{
    theme::{Effect, Style},
    utils::markup::StyledString,
};
use hifirs_qobuz_api::client::album::Album as QobuzAlbum;
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, str::FromStr};

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

impl From<QobuzAlbum> for Album {
    fn from(value: QobuzAlbum) -> Self {
        let year = chrono::NaiveDate::from_str(&value.release_date_original)
            .expect("failed to parse date")
            .format("%Y");

        let tracks = if let Some(tracks) = &value.tracks {
            tracks
                .items
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let mut track: Track = t.clone().into();

                    track.position = i + 1;

                    track
                })
                .collect::<VecDeque<Track>>()
        } else {
            VecDeque::new()
        };

        Self {
            id: value.id,
            title: value.title,
            artist: value.artist.into(),
            total_tracks: value.tracks_count as usize,
            release_year: year
                .to_string()
                .parse::<usize>()
                .expect("error converting year"),
            hires_available: value.hires_streamable,
            explicit: value.parental_warning,
            available: value.streamable,
            tracks,
            cover_art: value.image.large,
        }
    }
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
