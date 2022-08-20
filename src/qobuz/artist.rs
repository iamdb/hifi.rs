use serde::{Deserialize, Serialize};
use tui::layout::Constraint;

use crate::{
    qobuz::{album::Albums, Image},
    ui::components::{Row, TableHeaders, TableRows, TableWidths},
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

impl TableHeaders for ArtistSearchResults {
    fn headers(&self) -> Vec<String> {
        self.artists.headers()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artists {
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
    pub items: Vec<Artist>,
}

impl TableHeaders for Artists {
    fn headers(&self) -> Vec<String> {
        self.items.first().unwrap().headers()
    }
}

impl TableRows for Artists {
    fn rows(&self) -> Vec<Row> {
        self.items.iter().map(|t| t.into()).collect::<Vec<Row>>()
    }
}

impl TableWidths for Artists {
    fn widths(&self, size: u16) -> Vec<Constraint> {
        vec![Constraint::Length((size as f64 * 0.5) as u16)]
    }
}

impl From<Artists> for Vec<Vec<String>> {
    fn from(artists: Artists) -> Self {
        artists
            .items
            .into_iter()
            .map(|i| i.into())
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
    pub fn columns(&self) -> Vec<String> {
        vec![self.name.clone()]
    }
    pub fn row(&self, screen_width: u16) -> Vec<String> {
        let column_width = screen_width;
        let columns = self.columns();

        columns
            .into_iter()
            .map(|c| {
                if c.len() as u16 > column_width {
                    textwrap::fill(&c, column_width as usize)
                } else {
                    c
                }
            })
            .collect::<Vec<String>>()
    }
}

impl TableHeaders for Artist {
    fn headers(&self) -> Vec<String> {
        vec!["Name".to_string()]
    }
}

impl From<Artist> for Vec<String> {
    fn from(artist: Artist) -> Self {
        vec![artist.name, artist.id.to_string()]
    }
}

impl From<&Artist> for Vec<String> {
    fn from(artist: &Artist) -> Self {
        vec![artist.name.clone(), artist.id.to_string()]
    }
}

impl From<Artist> for Vec<Vec<String>> {
    fn from(artist: Artist) -> Self {
        vec![artist.into()]
    }
}

impl From<&Artist> for Row {
    fn from(artist: &Artist) -> Self {
        let strings: Vec<String> = artist.into();

        Row::new(strings)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtherArtists {
    pub id: i64,
    pub name: String,
    pub roles: Vec<String>,
}
