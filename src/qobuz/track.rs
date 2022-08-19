use serde::{Deserialize, Serialize};
use tui::{style::Style, widgets::Row as TermRow};

use crate::{
    qobuz::{album::Album, TrackURL},
    state::{AudioQuality, Bytes},
    ui::terminal::components::Row,
};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tracks {
    pub offset: i64,
    pub limit: i64,
    pub total: i64,
    pub items: Vec<Track>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub maximum_bit_depth: i64,
    pub copyright: Option<String>,
    pub performers: Option<String>,
    pub audio_info: AudioInfo,
    pub performer: Performer,
    pub album: Option<Album>,
    pub isrc: Option<String>,
    pub title: String,
    pub version: Option<String>,
    pub duration: i64,
    pub parental_warning: bool,
    pub track_number: i64,
    pub maximum_channel_count: i64,
    pub id: i32,
    pub media_number: i64,
    pub maximum_sampling_rate: f64,
    pub release_date_original: Option<String>,
    pub release_date_download: Option<String>,
    pub release_date_stream: Option<String>,
    pub purchasable: bool,
    pub streamable: bool,
    pub previewable: bool,
    pub sampleable: bool,
    pub downloadable: bool,
    pub displayable: bool,
    pub purchasable_at: Option<i64>,
    pub streamable_at: Option<i64>,
    pub hires: bool,
    pub hires_streamable: bool,
}

impl From<Track> for Vec<u8> {
    fn from(track: Track) -> Self {
        bincode::serialize(&track).expect("failed to serialize track")
    }
}

impl From<&Track> for Row<'_> {
    fn from(track: &Track) -> Self {
        let strings: Vec<String> = track.into();

        Row::new(TermRow::new(strings).style(Style::default()))
    }
}

impl From<&Track> for Vec<String> {
    fn from(track: &Track) -> Self {
        let mut fields = vec![track.title.clone()];

        if let Some(album) = &track.album {
            fields.push(album.title.clone());
        }

        fields
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PlaylistTrack {
    pub track: Track,
    pub quality: Option<AudioQuality>,
    pub track_url: Option<TrackURL>,
    pub album: Option<Album>,
}

impl From<Bytes> for PlaylistTrack {
    fn from(bytes: Bytes) -> Self {
        let deserialized: PlaylistTrack =
            bincode::deserialize(&bytes.vec()).expect("failed to deserialize playlist track");

        deserialized
    }
}

impl PlaylistTrack {
    pub fn new(track: Track, quality: Option<AudioQuality>, album: Option<Album>) -> Self {
        PlaylistTrack {
            track,
            quality,
            track_url: None,
            album,
        }
    }

    pub fn set_track_url(&mut self, track_url: TrackURL) -> Self {
        self.track_url = Some(track_url);
        self.clone()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioInfo {
    pub replaygain_track_gain: f64,
    pub replaygain_track_peak: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Performer {
    pub id: i64,
    pub name: String,
}
