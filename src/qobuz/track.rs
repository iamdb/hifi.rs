use crate::{
    qobuz::{album::Album, TrackURL},
    state::{AudioQuality, Bytes},
    ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths},
};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tracks {
    pub offset: i64,
    pub limit: i64,
    pub total: i64,
    pub items: Vec<Track>,
}

impl TableRows for Tracks {
    fn rows(&self) -> Vec<Row> {
        self.items.iter().map(|i| i.row()).collect::<Vec<Row>>()
    }
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
    fn columns(&self) -> Vec<String> {
        vec![
            self.track_number.to_string(),
            self.title.clone(),
            self.performer.name.clone(),
        ]
    }
}

impl TableHeaders for Track {
    fn headers() -> Vec<String> {
        vec!["#".to_string(), "Title".to_string(), "Artist".to_string()]
    }
}

impl TableWidths for Track {
    fn widths() -> Vec<ColumnWidth> {
        vec![
            ColumnWidth::new(8),
            ColumnWidth::new(52),
            ColumnWidth::new(40),
        ]
    }
}

impl From<Track> for Vec<u8> {
    fn from(track: Track) -> Self {
        bincode::serialize(&track).expect("failed to serialize track")
    }
}

impl TableRow for Track {
    fn row(&self) -> Row {
        Row::new(self.columns(), Track::widths())
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
