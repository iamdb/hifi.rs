use crate::ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths};
use qobuz_client::client::{
    playlist::{Playlist, Playlists, UserPlaylistsResult},
    track::Track,
};

impl TableHeaders for UserPlaylistsResult {
    fn headers() -> Vec<String> {
        Playlists::headers()
    }
}

impl TableRows for Playlists {
    fn rows(&self) -> Vec<Row> {
        self.items.iter().map(|t| t.row()).collect::<Vec<Row>>()
    }
}

impl TableHeaders for Playlists {
    fn headers() -> Vec<String> {
        vec!["Title".to_string()]
    }
}

impl TableRow for Playlist {
    fn row(&self) -> Row {
        Row::new(vec![self.name.clone()], Playlist::widths())
    }
}

impl TableRows for Playlist {
    fn rows(&self) -> Vec<Row> {
        if let Some(tracks) = &self.tracks {
            tracks
                .items
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let mut row = t.row();
                    row.remove_column(0);
                    row.insert_column(0, (i + 1).to_string());

                    row
                })
                .collect::<Vec<Row>>()
        } else {
            vec![]
        }
    }
}

impl TableWidths for Playlist {
    fn widths() -> Vec<ColumnWidth> {
        Track::widths()
    }
}
