use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{album::Albums, artist::Artist, playlist::Playlists, track::Track, Image};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchAllResults {
    pub query: String,
    pub albums: Albums,
    pub tracks: Tracks,
    pub artists: Artists,
    pub playlists: Playlists,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Analytics {
    #[serde(rename = "search_external_id")]
    pub search_external_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tracks {
    pub limit: i64,
    pub offset: i64,
    pub analytics: Analytics,
    pub total: i64,
    pub items: Vec<Track>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artists {
    pub limit: i64,
    pub offset: i64,
    pub analytics: Analytics,
    pub total: i64,
    pub items: Vec<Artist>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeaturedArtist {
    pub id: i64,
    pub name: String,
    pub slug: String,
    #[serde(rename = "albums_count")]
    pub albums_count: i64,
    pub picture: Value,
    pub image: Image,
}
