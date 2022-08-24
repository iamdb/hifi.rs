use serde::{Deserialize, Serialize};

use crate::{
    qobuz::{album::Albums, Image},
    ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths},
};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistSearchResults {
    pub query: String,
    pub artists: Artists,
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

impl TableRows for Artists {
    fn rows(&self) -> Vec<Row> {
        self.items.iter().map(|t| t.row()).collect::<Vec<Row>>()
    }
}

impl From<Artists> for Vec<Vec<String>> {
    fn from(artists: Artists) -> Self {
        artists
            .items
            .into_iter()
            .map(|i| i.columns())
            .collect::<Vec<Vec<String>>>()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub image: Option<Image>,
    pub name: String,
    pub id: i64,
    pub albums_count: i64,
    pub slug: String,
    pub albums: Option<Albums>,
}

impl Artist {
    fn columns(&self) -> Vec<String> {
        vec![self.name.clone()]
    }
}

impl TableWidths for Artist {
    fn widths() -> Vec<ColumnWidth> {
        vec![ColumnWidth::new(100)]
    }
}

impl TableRow for Artist {
    fn row(&self) -> Row {
        Row::new(self.columns(), Artist::widths())
    }
}

impl TableHeaders for Artist {
    fn headers() -> Vec<String> {
        vec!["Name".to_string()]
    }
}

impl From<Artist> for Vec<String> {
    fn from(artist: Artist) -> Self {
        artist.columns()
    }
}

impl From<Artist> for Vec<Vec<String>> {
    fn from(artist: Artist) -> Self {
        vec![artist.into()]
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtherArtists {
    pub id: i64,
    pub name: String,
    pub roles: Vec<String>,
}
