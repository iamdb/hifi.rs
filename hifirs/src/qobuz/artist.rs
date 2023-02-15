use crate::ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths};
use hifirs_qobuz_api::client::artist::{Artist, Artists};

impl TableRows for Artists {
    fn rows(&self) -> Vec<Row> {
        self.items.iter().map(|t| t.row()).collect::<Vec<Row>>()
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
