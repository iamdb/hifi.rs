use crate::cursive::CursiveFormat;
use cursive::{
    theme::{Effect, Style},
    utils::markup::StyledString,
};
use hifirs_qobuz_api::client::album::Album;
use std::str::FromStr;

impl CursiveFormat for Album {
    fn list_item(&self) -> StyledString {
        let mut style = Style::none();

        if !self.streamable {
            style = style.combine(Effect::Dim).combine(Effect::Strikethrough);
        }

        let mut title = StyledString::styled(self.title.clone(), style.combine(Effect::Bold));

        title.append_styled(" by ", style);
        title.append_styled(self.artist.name.clone(), style);
        title.append_styled(" ", style);

        let year = chrono::NaiveDate::from_str(&self.release_date_original)
            .expect("failed to parse date")
            .format("%Y");

        title.append_styled(year.to_string(), style.combine(Effect::Dim));
        title.append_plain(" ");

        if self.parental_warning {
            title.append_styled("e", style.combine(Effect::Dim));
        }

        if self.hires_streamable {
            title.append_styled("*", style.combine(Effect::Dim));
        }

        title
    }
}
