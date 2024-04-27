use crate::service::{Album, Artist, Track, TrackStatus};
use hifirs_qobuz_api::client::track::Track as QobuzTrack;

impl From<QobuzTrack> for Track {
    fn from(value: QobuzTrack) -> Self {
        let album = value.album.as_ref().map(|a| {
            let album: Album = a.into();

            album
        });

        let artist = if let Some(p) = &value.performer {
            Some(Artist {
                id: p.id as u32,
                name: p.name.clone(),
                albums: None,
            })
        } else {
            value.album.as_ref().map(|a| a.clone().artist.into())
        };

        let cover_art = value.album.as_ref().map(|a| a.image.large.clone());

        let status = if value.streamable {
            TrackStatus::Unplayed
        } else {
            TrackStatus::Unplayable
        };

        Self {
            id: value.id as u32,
            number: value.track_number as u32,
            title: value.title,
            album,
            artist,
            duration_seconds: value.duration as u32,
            explicit: value.parental_warning,
            hires_available: value.hires_streamable,
            sampling_rate: value.maximum_sampling_rate.unwrap_or(0.0) as f32,
            bit_depth: value.maximum_bit_depth as u32,
            status,
            track_url: None,
            available: value.streamable,
            position: value.position.unwrap_or(value.track_number as usize) as u32,
            cover_art,
            media_number: value.media_number as u32,
        }
    }
}

impl From<&QobuzTrack> for Track {
    fn from(value: &QobuzTrack) -> Self {
        value.clone().into()
    }
}
