pub mod client;

use serde_derive::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistSearchResults {
    pub query: String,
    pub artists: Artists,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artists {
    pub limit: i64,
    pub offset: i64,
    pub analytics: Analytics,
    pub total: i64,
    pub items: Vec<Artist>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumSearchResults {
    pub query: String,
    pub albums: Albums,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Albums {
    pub limit: i64,
    pub offset: i64,
    pub analytics: Analytics,
    pub total: i64,
    pub items: Vec<Album>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Analytics {
    pub search_external_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackInfo {
    pub maximum_bit_depth: i64,
    pub copyright: String,
    pub performers: String,
    pub audio_info: AudioInfo,
    pub performer: Performer,
    pub album: Album,
    pub work: Value,
    pub isrc: String,
    pub title: String,
    pub version: String,
    pub duration: i64,
    pub parental_warning: bool,
    pub track_number: i64,
    pub maximum_channel_count: i64,
    pub id: i64,
    pub media_number: i64,
    pub maximum_sampling_rate: i64,
    pub articles: Vec<Value>,
    pub release_date_original: Value,
    pub release_date_download: Value,
    pub release_date_stream: Value,
    pub purchasable: bool,
    pub streamable: bool,
    pub previewable: bool,
    pub sampleable: bool,
    pub downloadable: bool,
    pub displayable: bool,
    pub purchasable_at: i64,
    pub streamable_at: i64,
    pub hires: bool,
    pub hires_streamable: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioInfo {
    pub replaygain_track_gain: f64,
    pub replaygain_track_peak: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Performer {
    pub id: i64,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Album {
    pub area: Option<Value>,
    pub articles: Option<Vec<Value>>,
    pub artist: Artist,
    pub artists: Vec<OtherArtists>,
    pub awards: Option<Vec<Value>>,
    pub catchline: Option<String>,
    pub copyright: Option<String>,
    pub created_at: Option<i64>,
    pub description: Option<String>,
    pub displayable: bool,
    pub downloadable: bool,
    pub duration: i64,
    pub genre: Genre,
    pub genres_list: Option<Vec<String>>,
    pub goodies: Option<Vec<Value>>,
    pub hires: bool,
    pub hires_streamable: bool,
    pub id: String,
    pub image: Image,
    pub is_official: Option<bool>,
    pub label: Label,
    pub maximum_bit_depth: Option<i64>,
    pub maximum_channel_count: Option<i64>,
    pub maximum_sampling_rate: Option<f64>,
    pub maximum_technical_specifications: Option<String>,
    pub media_count: Option<i64>,
    pub parental_warning: bool,
    pub period: Option<Value>,
    pub popularity: i64,
    pub previewable: bool,
    pub product_sales_factors_monthly: Option<f64>,
    pub product_sales_factors_weekly: Option<f64>,
    pub product_sales_factors_yearly: Option<f64>,
    pub product_type: Option<String>,
    pub product_url: Option<String>,
    pub purchasable: bool,
    pub purchasable_at: Option<i64>,
    pub qobuz_id: i64,
    pub recording_information: Option<String>,
    pub relative_url: Option<String>,
    pub release_date_download: String,
    pub release_date_original: String,
    pub release_date_stream: String,
    pub release_tags: Option<Vec<String>>,
    pub release_type: Option<String>,
    pub released_at: Option<i64>,
    pub sampleable: bool,
    pub slug: Option<String>,
    pub streamable: bool,
    pub streamable_at: i64,
    pub subtitle: Option<String>,
    pub title: String,
    pub tracks_count: i64,
    pub upc: String,
    pub url: String,
    pub version: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub small: String,
    pub thumbnail: String,
    pub large: String,
    pub back: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub image: Value,
    pub name: String,
    pub id: i64,
    pub albums_count: i64,
    pub slug: String,
    pub picture: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtherArtists {
    pub id: i64,
    pub name: String,
    pub roles: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub id: i64,
    pub albums_count: i64,
    pub supplier_id: i64,
    pub slug: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genre {
    pub path: Vec<i64>,
    pub color: String,
    pub name: String,
    pub id: i64,
    pub slug: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackURL {
    pub track_id: i64,
    pub duration: i64,
    pub url: String,
    pub format_id: i64,
    pub mime_type: String,
    pub sampling_rate: f64,
    pub bit_depth: i64,
}
