use crate::{
    cursive::CursiveFormat,
    ui::components::{ColumnWidth, Row, TableHeaders, TableRow, TableRows, TableWidths},
};
use cursive::{
    theme::{Effect, Style},
    utils::markup::StyledString,
};
use gstreamer::ClockTime;
use hifirs_qobuz_api::client::{
    playlist::TrackListTracks,
    track::{Track, TrackListTrack, TrackStatus, Tracks},
};

impl CursiveFormat for Track {
    fn list_item(&self) -> StyledString {
        let mut title = StyledString::styled(self.title.clone(), Effect::Bold);

        if let Some(performer) = &self.performer {
            title.append_plain(" by ");
            title.append_plain(performer.name.clone());
        }

        let duration = ClockTime::from_seconds(self.duration as u64)
            .to_string()
            .as_str()[2..7]
            .to_string();
        title.append_plain(" ");
        title.append_styled(duration, Effect::Dim);
        title.append_plain(" ");

        if self.parental_warning {
            title.append_styled("e", Effect::Dim);
        }

        if self.hires_streamable {
            title.append_styled("*", Effect::Dim);
        }

        title
    }
    fn track_list_item(&self, inactive: bool, index: Option<usize>) -> StyledString {
        let mut style = Style::none();

        if inactive {
            style = style.combine(Effect::Dim).combine(Effect::Italic);
        }

        let num = if let Some(index) = index {
            index
        } else {
            self.track_number as usize
        };

        let mut item = StyledString::styled(format!("{:02} ", num), style);
        item.append_styled(self.title.trim(), style.combine(Effect::Bold));
        item.append_plain(" ");

        let duration = ClockTime::from_seconds(self.duration as u64)
            .to_string()
            .as_str()[2..7]
            .to_string();

        item.append_styled(duration, style.combine(Effect::Dim));

        item
    }
}

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
            ColumnWidth::new(6),
            ColumnWidth::new(44),
            ColumnWidth::new(35),
            ColumnWidth::new(15),
        ]
    }
}

impl TableRow for TrackListTrack {
    fn row(&self) -> Row {
        let mut row = Row::new(self.columns(), Track::widths());

        if self.status == TrackStatus::Played {
            row.set_dim(true);
        }

        row
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
