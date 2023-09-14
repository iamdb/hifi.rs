use std::collections::BTreeMap;

use hifirs_qobuz_api::client::playlist::Playlist as QobuzPlaylist;

use crate::service::{Playlist, Track};

impl From<QobuzPlaylist> for Playlist {
    fn from(value: QobuzPlaylist) -> Self {
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
