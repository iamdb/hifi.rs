use crate::service::{Album, Artist, Track, TrackStatus};
use hifirs_qobuz_api::client::track::Track as QobuzTrack;

impl From<QobuzTrack> for Track {
    fn from(value: QobuzTrack) -> Self {
        let album = if let Some(album) = &value.album {
            let a: Album = album.clone().into();
            Some(a)
        } else {
            None
        };

        let artist = if let Some(p) = &value.performer {
            Some(Artist {
                id: p.id as usize,
                name: p.name.clone(),
                albums: None,
            })
        } else {
            value.album.as_ref().map(|a| a.artist.clone().into())
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
            artist,
            duration_seconds: value.duration as usize,
            explicit: value.parental_warning,
            hires_available: value.hires_streamable,
            sampling_rate: value.maximum_sampling_rate as f32,
            bit_depth: value.maximum_bit_depth as usize,
            status,
            track_url: None,
            available: value.streamable,
            position: value.position.unwrap_or(value.track_number as usize),
            cover_art,
            media_number: value.media_number as usize,
        }
    }
}
