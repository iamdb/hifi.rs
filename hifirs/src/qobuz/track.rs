use crate::ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths};
use qobuz_client::client::{
    playlist::TrackListTracks,
    track::{Track, TrackListTrack, Tracks},
};

impl TableRow for Track {
    fn row(&self) -> Row {
        Row::new(self.columns(), Track::widths())
    }
}

impl TableRows for Tracks {
    fn rows(&self) -> Vec<Row> {
        self.items.iter().map(|i| i.row()).collect::<Vec<Row>>()
    }
}

impl TableHeaders for Track {
    fn headers() -> Vec<String> {
        vec![
            "#".to_string(),
            "Title".to_string(),
            "Artist".to_string(),
            "Len".to_string(),
        ]
    }
}

impl TableWidths for Track {
    fn widths() -> Vec<ColumnWidth> {
        vec![
            ColumnWidth::new(4),
            ColumnWidth::new(46),
            ColumnWidth::new(36),
            ColumnWidth::new(14),
        ]
    }
}

impl TableRow for TrackListTrack {
    fn row(&self) -> Row {
        Row::new(self.columns(), Track::widths())
    }
}

impl TableRows for TrackListTracks {
    fn rows(&self) -> Vec<Row> {
        self.iter().map(|i| i.row()).collect::<Vec<Row>>()
    }
}

impl TableHeaders for TrackListTrack {
    fn headers() -> Vec<String> {
        vec![
            "#".to_string(),
            "Title".to_string(),
            "Artist".to_string(),
            "Len".to_string(),
        ]
    }
}
