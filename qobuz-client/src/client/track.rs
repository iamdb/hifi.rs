use gstreamer::ClockTime;
use serde::{Deserialize, Serialize};

use crate::client::{album::Album, AudioQuality, TrackURL};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tracks {
    pub offset: i64,
    pub limit: i64,
    pub total: i64,
    pub items: Vec<Track>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub album: Option<Album>,
    pub audio_info: AudioInfo,
    pub copyright: Option<String>,
    pub displayable: bool,
    pub downloadable: bool,
    pub duration: i64,
    pub hires: bool,
    pub hires_streamable: bool,
    pub id: i32,
    pub isrc: Option<String>,
    pub maximum_bit_depth: i64,
    pub maximum_channel_count: i64,
    pub maximum_sampling_rate: f64,
    pub media_number: i64,
    pub parental_warning: bool,
    pub performer: Performer,
    pub performers: Option<String>,
    pub previewable: bool,
    pub purchasable: bool,
    pub purchasable_at: Option<i64>,
    pub release_date_download: Option<String>,
    pub release_date_original: Option<String>,
    pub release_date_stream: Option<String>,
    pub sampleable: bool,
    pub streamable: bool,
    pub streamable_at: Option<i64>,
    pub title: String,
    pub track_number: i64,
    pub version: Option<String>,
}

impl Track {
    pub fn columns(&self) -> Vec<String> {
        let duration = ClockTime::from_seconds(self.duration as u64)
            .to_string()
            .as_str()[3..7]
            .to_string();

        vec![
            self.track_number.to_string(),
            self.title.clone(),
            self.performer.name.clone(),
            duration,
        ]
    }
}

impl From<Track> for Vec<u8> {
    fn from(track: Track) -> Self {
        bincode::serialize(&track).expect("failed to serialize track")
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PlaylistTrack {
    pub track: Track,
    pub quality: Option<AudioQuality>,
    pub track_url: Option<TrackURL>,
    pub album: Option<Album>,
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
