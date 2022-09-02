use hifi_rs::{
    state::AudioQuality,
    ui::components::{ColumnWidth, Item, Row, TableHeaders, TableRow, TableRows, TableWidths},
};
use serde::{Deserialize, Serialize};
use tui::{
    style::{Color, Modifier, Style},
    text::Text,
    widgets::ListItem,
};

use crate::client::{
    api::Client,
    artist::{Artist, OtherArtists},
    track::{PlaylistTrack, Tracks},
    Composer, Image,
};

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

impl TableWidths for Album {
    fn widths() -> Vec<ColumnWidth> {
        vec![
            ColumnWidth::new(44),
            ColumnWidth::new(44),
            ColumnWidth::new(12),
        ]
    }
}

impl TableHeaders for Album {
    fn headers() -> Vec<String> {
        vec!["Title", "Artist", "Year"]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
    }
}

impl TableRow for Album {
    fn row(&self) -> Row {
        Row::new(self.columns(), Album::widths())
    }
}

impl Album {
    pub fn to_playlist_tracklist(&self, quality: AudioQuality) -> Option<Vec<PlaylistTrack>> {
        self.tracks.as_ref().map(|t| {
            t.items
                .iter()
                .map(|i| PlaylistTrack::new(i.clone(), Some(quality.clone()), Some(self.clone())))
                .collect::<Vec<PlaylistTrack>>()
        })
    }
    pub async fn attach_tracks(&mut self, client: Client) {
        if let Ok(album) = client.album(self.id.clone()).await {
            self.tracks = album.tracks;
        }
    }

    fn columns(&self) -> Vec<String> {
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

impl TableRows for Albums {
    fn rows(&self) -> Vec<Row> {
        self.items.iter().map(|t| t.row()).collect::<Vec<Row>>()
    }
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
    pub fn item_list(&self, max_width: usize, dim: bool) -> Vec<Item<'static>> {
        self.items
            .iter()
            .map(|t| {
                let title = textwrap::wrap(
                    format!("{} - {}", t.title.as_str(), t.artist.name).as_str(),
                    max_width,
                )
                .join("\n  ");

                let mut style = Style::default().fg(Color::White);

                if dim {
                    style = style.add_modifier(Modifier::DIM);
                }

                ListItem::new(Text::raw(title)).style(style).into()
            })
            .collect::<Vec<Item>>()
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
