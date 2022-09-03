use crate::ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths};
use qobuz_client::client::playlist::{Playlist, Playlists, UserPlaylistsResult};

impl TableHeaders for UserPlaylistsResult {
    fn headers() -> Vec<String> {
        vec!["Title".to_string()]
    }
}

impl TableRows for Playlists {
    fn rows(&self) -> Vec<Row> {
        self.items.iter().map(|t| t.into()).collect::<Vec<Row>>()
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
