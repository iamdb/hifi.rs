use crate::{
    cursive::CursiveFormat,
    ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths},
};
use cursive::{theme::Effect, utils::markup::StyledString};
use hifirs_qobuz_api::client::album::{Album, Albums};
use std::str::FromStr;

impl CursiveFormat for Album {
    fn list_item(&self) -> StyledString {
        let mut title = StyledString::styled(self.title.clone(), Effect::Bold);
        title.append_plain(" by ");
        title.append_plain(self.artist.name.clone());
        title.append_plain(" ");

        let year = chrono::NaiveDate::from_str(&self.release_date_original)
            .expect("failed to parse date")
            .format("%Y");

        title.append_styled(year.to_string(), Effect::Dim);
        title.append_plain(" ");

        if self.parental_warning {
            title.append_styled("e", Effect::Dim);
        }

        if self.hires_streamable {
            title.append_styled("*", Effect::Dim);
        }

        title
    }
}

impl TableRows for Albums {
    fn rows(&self) -> Vec<Row> {
        self.items.iter().map(|t| t.row()).collect::<Vec<Row>>()
    }
}

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
