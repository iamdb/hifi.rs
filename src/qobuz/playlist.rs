use serde::{Deserialize, Serialize};
use sled::IVec;
use tui::{layout::Constraint, style::Style, widgets::Row as TermRow};

use crate::{
    qobuz::{
        track::{PlaylistTrack, Tracks},
        User,
    },
    ui::terminal::components::{Row, TableHeaders, TableRows, TableWidths},
};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPlaylistsResult {
    pub user: User,
    pub playlists: Playlists,
}

impl TableHeaders for UserPlaylistsResult {
    fn headers(&self) -> Vec<String> {
        self.playlists.headers()
    }
}

impl From<UserPlaylistsResult> for Vec<Vec<String>> {
    fn from(playlist: UserPlaylistsResult) -> Self {
        vec![playlist.into()]
    }
}

impl From<UserPlaylistsResult> for Vec<String> {
    fn from(playlist: UserPlaylistsResult) -> Self {
        playlist.into()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<Playlists> for Vec<Vec<String>> {
    fn from(playlists: Playlists) -> Self {
        playlists
            .items
            .into_iter()
            .map(|i| vec![i.name, i.id.to_string()])
            .collect::<Vec<Vec<String>>>()
    }
}

impl TableRows for Playlists {
    fn rows<'a>(&self) -> Vec<Row<'a>> {
        self.items
            .iter()
            .map(|t| t.into())
            .collect::<Vec<Row<'a>>>()
    }
}

impl TableHeaders for Playlists {
    fn headers(&self) -> Vec<String> {
        if let Some(first) = self.items.first() {
            first.headers()
        } else {
            vec![]
        }
    }
}

impl From<Playlist> for Vec<String> {
    fn from(playlist: Playlist) -> Self {
        vec![playlist.name, playlist.id.to_string()]
    }
}

impl From<&Playlist> for Vec<String> {
    fn from(playlist: &Playlist) -> Self {
        vec![playlist.name.clone(), playlist.id.to_string()]
    }
}

impl From<Playlist> for Vec<Vec<String>> {
    fn from(playlist: Playlist) -> Self {
        vec![playlist.into()]
    }
}

impl From<&Playlist> for Row<'_> {
    fn from(playlist: &Playlist) -> Self {
        let strings: Vec<String> = playlist.into();

        Row::new(TermRow::new(strings).style(Style::default()))
    }
}

impl TableHeaders for Playlist {
    fn headers(&self) -> Vec<String> {
        vec!["Title".to_string()]
    }
}

impl TableRows for Playlist {
    fn rows<'a>(&self) -> Vec<Row<'a>> {
        if let Some(tracks) = &self.tracks {
            tracks
                .items
                .iter()
                .map(|i| i.into())
                .collect::<Vec<Row<'a>>>()
        } else {
            vec![]
        }
    }
}

impl TableWidths for Playlists {
    fn widths(&self, size: u16) -> Vec<Constraint> {
        vec![
            Constraint::Length((size as f64 * 0.5) as u16),
            Constraint::Length((size as f64 * 0.4) as u16),
            Constraint::Length((size as f64 * 0.1) as u16),
        ]
    }
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
