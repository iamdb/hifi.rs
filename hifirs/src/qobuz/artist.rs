use crate::{
    cursive::CursiveFormat,
    ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths},
};
use cursive::utils::markup::StyledString;
use hifirs_qobuz_api::client::artist::{Artist, Artists};

impl CursiveFormat for Artist {
    fn list_item(&self) -> StyledString {
        StyledString::plain(self.name.clone())
    }
}

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
