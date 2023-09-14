use std::collections::BTreeMap;

use hifirs_qobuz_api::client::playlist::Playlist as QobuzPlaylist;

use crate::service::{Playlist, Track};

impl From<QobuzPlaylist> for Playlist {
    fn from(value: QobuzPlaylist) -> Self {
        let tracks = if let Some(tracks) = value.tracks {
            let mut position = 1_u32;

            tracks
                .items
                .into_iter()
                .filter_map(|t| {
                    if t.streamable {
                        let mut track: Track = t.into();

                        let next_position = position;
                        track.position = next_position;

                        position += 1;

                        Some((next_position, track))
                    } else {
                        None
                    }
                })
                .collect::<BTreeMap<u32, Track>>()
        } else {
            BTreeMap::new()
        };

        let cover_art = if let Some(image) = value.image_rectangle.first() {
            Some(image.clone())
        } else if let Some(images) = value.images300 {
            images.first().cloned()
        } else {
            None
        };

        Self {
            id: value.id as u32,
            title: value.name,
            duration_seconds: value.duration as u32,
            tracks_count: value.tracks_count as u32,
            cover_art,
            tracks,
        }
    }
}
