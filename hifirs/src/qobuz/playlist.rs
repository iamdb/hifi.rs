use std::collections::VecDeque;

use crate::qobuz::track::Track;
use hifirs_qobuz_api::client::playlist::Playlist as QobuzPlaylist;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub title: String,
    pub duration_seconds: usize,
    pub tracks_count: usize,
    pub id: usize,
    pub cover_art: String,
    pub tracks: VecDeque<Track>,
}

impl From<QobuzPlaylist> for Playlist {
    fn from(value: QobuzPlaylist) -> Self {
        let tracks = if let Some(tracks) = value.tracks {
            tracks
                .items
                .into_iter()
                .map(|t| t.into())
                .collect::<VecDeque<Track>>()
        } else {
            VecDeque::new()
        };

        Self {
            id: value.id as usize,
            title: value.name,
            duration_seconds: value.duration as usize,
            tracks_count: value.tracks_count as usize,
            cover_art: value.image_rectangle.first().unwrap().clone(),
            tracks,
        }
    }
}
