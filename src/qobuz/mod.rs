pub mod client;

use gstreamer::ClockTime;
use serde::{Deserialize, Serialize};
use sled::IVec;

use crate::{state::AudioQuality, state::Bytes};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistSearchResults {
    pub query: String,
    pub artists: Artists,
}

impl ArtistSearchResults {
    pub fn table_headers(&self) -> Vec<&str> {
        vec!["Arist Name", "ID"]
    }
}

impl From<ArtistSearchResults> for Vec<Vec<String>> {
    fn from(results: ArtistSearchResults) -> Self {
        results.artists.into()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artists {
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
    pub items: Vec<Artist>,
}

impl From<Artists> for Vec<Vec<String>> {
    fn from(artists: Artists) -> Self {
        artists
            .items
            .into_iter()
            .map(|i| vec![i.name, i.id.to_string()])
            .collect::<Vec<Vec<String>>>()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumSearchResults {
    pub query: String,
    pub albums: Albums,
}

impl AlbumSearchResults {
    pub fn table_headers(&self) -> Vec<&str> {
        vec![
            "Album title",
            "Arist Name",
            "Release Year",
            "Duration",
            "ID",
        ]
    }
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

impl From<Albums> for Vec<Vec<String>> {
    fn from(albums: Albums) -> Self {
        albums
            .items
            .into_iter()
            .map(|album| album.into())
            .collect::<Vec<Vec<String>>>()
    }
}

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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Performer {
    pub id: i64,
    pub name: String,
}

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
    pub fn table_headers(&self) -> Vec<&str> {
        vec!["Title", "Artist", "Release Date", "Duration"]
    }
    pub fn to_playlist_tracklist(&self, quality: AudioQuality) -> Option<Vec<PlaylistTrack>> {
        self.tracks.as_ref().map(|t| {
            t.items
                .iter()
                .map(|i| PlaylistTrack::new(i.clone(), Some(quality.clone()), Some(self.clone())))
                .collect::<Vec<PlaylistTrack>>()
        })
    }
}

impl From<Album> for Vec<String> {
    fn from(album: Album) -> Self {
        let mut fields = vec![album.title, album.artist.name, album.release_date_original];

        if let Some(duration) = album.duration {
            fields.push(
                ClockTime::from_seconds(duration as u64)
                    .to_string()
                    .as_str()[2..7]
                    .to_string(),
            );
        }

        fields.push(album.id);

        fields
    }
}

impl From<Album> for Vec<Vec<String>> {
    fn from(album: Album) -> Self {
        vec![album.into()]
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Composer {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub albums_count: i64,
    pub image: Option<Image>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub small: String,
    pub thumbnail: Option<String>,
    pub large: String,
    pub back: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub image: Option<Image>,
    pub name: String,
    pub id: i64,
    pub albums_count: i64,
    pub slug: String,
}

impl Artist {
    pub fn table_headers(&self) -> Vec<String> {
        vec!["Name".to_string(), "ID".to_string()]
    }
}

impl From<Artist> for Vec<String> {
    fn from(artist: Artist) -> Self {
        vec![artist.name, artist.id.to_string()]
    }
}

impl From<Artist> for Vec<Vec<String>> {
    fn from(artist: Artist) -> Self {
        vec![artist.into()]
    }
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
    pub track_id: i32,
    pub duration: i32,
    pub url: String,
    pub format_id: i32,
    pub mime_type: String,
    pub sampling_rate: f64,
    pub bit_depth: i32,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub login: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPlaylists {
    pub user: User,
    pub playlists: Playlists,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Owner {
    pub id: i64,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub owner: Owner,
    pub users_count: i64,
    pub images150: Vec<String>,
    pub images: Vec<String>,
    pub is_collaborative: bool,
    pub is_published: Option<bool>,
    pub description: String,
    pub created_at: i64,
    pub images300: Vec<String>,
    pub duration: i64,
    pub updated_at: i64,
    pub published_to: Option<i64>,
    pub tracks_count: i64,
    pub public_at: i64,
    pub name: String,
    pub is_public: bool,
    pub published_from: Option<i64>,
    pub id: i64,
    pub is_featured: bool,
    pub position: Option<i64>,
    #[serde(default)]
    pub image_rectangle_mini: Vec<String>,
    pub timestamp_position: Option<i64>,
    #[serde(default)]
    pub image_rectangle: Vec<String>,
    pub slug: Option<String>,
    #[serde(default)]
    pub stores: Vec<String>,
    pub tracks: Option<Tracks>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlists {
    pub offset: i64,
    pub limit: i64,
    pub total: i64,
    pub items: Vec<Playlist>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTracks(Vec<PlaylistTrack>);

impl From<IVec> for PlaylistTracks {
    fn from(ivec: IVec) -> Self {
        let deserialized: PlaylistTracks =
            bincode::deserialize(&ivec).expect("failed to deserialize playlist tracks");

        deserialized
    }
}
