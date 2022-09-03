use qobuz_client::client::album::Album;

use crate::ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths};

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
