use serde::{Deserialize, Serialize};
use tui::{layout::Constraint, style::Style, widgets::Row as TermRow};

use crate::{
    qobuz::{album::Albums, Image},
    ui::terminal::components::{Row, TableHeaders, TableRows, TableWidths},
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
    fn rows<'a>(&self) -> Vec<Row<'a>> {
        self.items
            .iter()
            .map(|t| t.into())
            .collect::<Vec<Row<'a>>>()
    }
}

impl TableWidths for Artists {
    fn widths(&self, size: u16) -> Vec<Constraint> {
        vec![
            Constraint::Length((size as f64 * 0.5) as u16),
            Constraint::Length((size as f64 * 0.4) as u16),
            Constraint::Length((size as f64 * 0.1) as u16),
        ]
    }
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
pub struct Artist {
    pub image: Option<Image>,
    pub name: String,
    pub id: i64,
    pub albums_count: i64,
    pub slug: String,
    pub albums: Option<Albums>,
}

impl TableHeaders for Artist {
    fn headers(&self) -> Vec<String> {
        vec!["Name".to_string(), "ID".to_string()]
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

impl From<&Artist> for Row<'_> {
    fn from(artist: &Artist) -> Self {
        let strings: Vec<String> = artist.into();

        Row::new(TermRow::new(strings).style(Style::default()))
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtherArtists {
    pub id: i64,
    pub name: String,
    pub roles: Vec<String>,
}
