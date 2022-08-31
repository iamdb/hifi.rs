use crate::{
    qobuz::{
        track::{PlaylistTrack, Tracks},
        User,
    },
    ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths},
};
use serde::{Deserialize, Serialize};
use sled::IVec;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPlaylistsResult {
    pub user: User,
    pub playlists: Playlists,
}

impl TableHeaders for UserPlaylistsResult {
    fn headers() -> Vec<String> {
        vec!["Title".to_string()]
    }
}

impl From<UserPlaylistsResult> for Vec<Vec<String>> {
    fn from(playlist: UserPlaylistsResult) -> Self {
        vec![playlist.into()]
    }
}

impl From<UserPlaylistsResult> for Vec<String> {
    fn from(playlist: UserPlaylistsResult) -> Self {
        playlist
            .playlists
            .items
            .iter()
            .map(|i| i.name.to_string())
            .collect::<Vec<String>>()
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

impl Playlist {
    pub fn reverse(&mut self) {
        if let Some(tracks) = &mut self.tracks {
            tracks.items.reverse();
        }
    }
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
            .map(|i| vec![i.name])
            .collect::<Vec<Vec<String>>>()
    }
}

impl TableRows for Playlists {
    fn rows(&self) -> Vec<Row> {
        self.items.iter().map(|t| t.into()).collect::<Vec<Row>>()
    }
}

impl From<Playlist> for Vec<String> {
    fn from(playlist: Playlist) -> Self {
        vec![playlist.name]
    }
}

impl From<Box<Playlist>> for Vec<String> {
    fn from(playlist: Box<Playlist>) -> Self {
        vec![playlist.name]
    }
}

impl From<&Playlist> for Vec<String> {
    fn from(playlist: &Playlist) -> Self {
        vec![playlist.name.clone()]
    }
}

impl From<Box<Playlist>> for Vec<Vec<String>> {
    fn from(playlist: Box<Playlist>) -> Self {
        vec![playlist.into()]
    }
}

impl From<&Playlist> for Row {
    fn from(playlist: &Playlist) -> Self {
        let strings: Vec<String> = playlist.into();

        Row::new(strings, Playlist::widths())
    }
}

impl TableHeaders for Playlist {
    fn headers() -> Vec<String> {
        vec!["Title".to_string()]
    }
}

impl TableRows for Playlist {
    fn rows(&self) -> Vec<Row> {
        if let Some(tracks) = &self.tracks {
            tracks.items.iter().map(|i| i.row()).collect::<Vec<Row>>()
        } else {
            vec![]
        }
    }
}

impl TableWidths for Playlist {
    fn widths() -> Vec<ColumnWidth> {
        vec![ColumnWidth::new(50), ColumnWidth::new(50)]
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
