use hifirs_qobuz_api::client::album::Album as QobuzAlbum;
use std::{collections::BTreeMap, str::FromStr};

use crate::service::{Album, Track};

impl From<QobuzAlbum> for Album {
    fn from(value: QobuzAlbum) -> Self {
        let year = chrono::NaiveDate::from_str(&value.release_date_original)
            .expect("failed to parse date")
            .format("%Y");

        let tracks = if let Some(tracks) = value.tracks {
            tracks
                .items
                .into_iter()
                .enumerate()
                .map(|(i, t)| {
                    let mut track: Track = t.into();

                    let position = (i + 1) as u32;
                    track.position = position;

                    (position, track)
                })
                .collect::<BTreeMap<u32, Track>>()
        } else {
            BTreeMap::new()
        };

        Self {
            id: value.id,
            title: value.title,
            artist: value.artist.into(),
            total_tracks: value.tracks_count as u32,
            release_year: year
                .to_string()
                .parse::<u32>()
                .expect("error converting year"),
            hires_available: value.hires_streamable,
            explicit: value.parental_warning,
            available: value.streamable,
            tracks,
            cover_art: value.image.large,
        }
    }
}
