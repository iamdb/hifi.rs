use hifirs_qobuz_api::client::album::Album as QobuzAlbum;
use std::{collections::VecDeque, str::FromStr};

use crate::service::{Album, Track};

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
