use std::collections::VecDeque;

use crate::client::{
    api::Client,
    artist::{Artist, OtherArtists},
    track::{TrackListTrack, TrackStatus, Tracks},
    AudioQuality, Composer, Image,
};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Album {
    pub artist: Artist,
    pub artists: Option<Vec<OtherArtists>>,
    pub catchline: Option<String>,
    pub composer: Option<Composer>,
    pub copyright: Option<String>,
    pub created_at: Option<i64>,
    pub description: Option<String>,
    pub displayable: bool,
    pub downloadable: bool,
    pub duration: Option<i64>,
    pub genre: Genre,
    pub genres_list: Option<Vec<String>>,
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
    pub popularity: Option<i64>,
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
    pub streamable_at: Option<i64>,
    pub subtitle: Option<String>,
    pub title: String,
    pub tracks: Option<Tracks>,
    pub tracks_count: i64,
    pub upc: String,
    pub url: Option<String>,
    pub version: Option<String>,
}

impl Album {
    pub fn to_tracklist(&self, quality: AudioQuality) -> Option<VecDeque<TrackListTrack>> {
        self.tracks.as_ref().map(|t| {
            t.items
                .iter()
                .filter_map(|t| {
                    if t.streamable {
                        let mut track = TrackListTrack::new(
                            t.clone(),
                            Some(t.track_number as usize),
                            Some(self.tracks_count as usize),
                            Some(quality.clone()),
                            Some(self.clone()),
                        );

                        if t.track_number == 1 {
                            track.status = TrackStatus::Playing;
                        }

                        Some(track)
                    } else {
                        None
                    }
                })
                .collect::<VecDeque<TrackListTrack>>()
        })
    }
    pub async fn attach_tracks(&mut self, client: Client) {
        debug!("attaching tracks to album");
        if let Ok(album) = client.album(&self.id).await {
            self.tracks = album.tracks;
        }
    }

    pub fn columns(&self) -> Vec<String> {
        let hires_icon = if self.hires_streamable { "*" } else { "" };
        let parental_icon = if self.parental_warning { "e" } else { "" };

        vec![
            format!("{} {}{}", self.title, hires_icon, parental_icon),
            self.artist.name.clone(),
            self.release_date_original.as_str()[0..4].to_string(),
        ]
    }
}

impl From<Box<Album>> for Vec<Vec<String>> {
    fn from(album: Box<Album>) -> Self {
        vec![album.columns()]
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumSearchResults {
    pub query: String,
    pub albums: Albums,
}

impl From<AlbumSearchResults> for Vec<Vec<String>> {
    fn from(results: AlbumSearchResults) -> Self {
        results.albums.into()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Albums {
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
    pub items: Vec<Album>,
}

impl Albums {
    pub fn sort_by_date(&mut self) {
        self.items.sort_by(|a, b| {
            chrono::NaiveDate::parse_from_str(a.release_date_original.as_str(), "%Y-%m-%d")
                .unwrap()
                .cmp(
                    &chrono::NaiveDate::parse_from_str(
                        b.release_date_original.as_str(),
                        "%Y-%m-%d",
                    )
                    .unwrap(),
                )
        });
    }
}

impl From<Albums> for Vec<Vec<String>> {
    fn from(albums: Albums) -> Self {
        albums
            .items
            .into_iter()
            .map(|album| album.columns())
            .collect::<Vec<Vec<String>>>()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub id: i64,
    pub albums_count: i64,
    pub supplier_id: i64,
    pub slug: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genre {
    pub path: Vec<i64>,
    pub color: String,
    pub name: String,
    pub id: i64,
    pub slug: String,
}
