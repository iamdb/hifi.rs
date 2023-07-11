use crate::cursive::CursiveFormat;
use cursive::{
    theme::{Effect, Style},
    utils::markup::StyledString,
};
use gstreamer::ClockTime;
use hifirs_qobuz_api::client::track::Track;

impl CursiveFormat for Track {
    fn list_item(&self) -> StyledString {
        let mut style = Style::none();

        if !self.streamable {
            style = style.combine(Effect::Dim).combine(Effect::Strikethrough);
        }

        let mut title = StyledString::styled(self.title.trim(), style.combine(Effect::Bold));

        if let Some(performer) = &self.performer {
            title.append_styled(" by ", style);
            title.append_styled(performer.name.trim(), style);
        }

        let duration = ClockTime::from_seconds(self.duration as u64)
            .to_string()
            .as_str()[2..7]
            .to_string();
        title.append_plain(" ");
        title.append_styled(duration, style.combine(Effect::Dim));
        title.append_plain(" ");

        if self.parental_warning {
            title.append_styled("e", style.combine(Effect::Dim));
        }

        if self.hires_streamable {
            title.append_styled("*", style.combine(Effect::Dim));
        }

        title
    }
    fn track_list_item(&self, inactive: bool, index: Option<usize>) -> StyledString {
        let mut style = Style::none();

        if inactive || !self.streamable {
            style = style
                .combine(Effect::Dim)
                .combine(Effect::Italic)
                .combine(Effect::Strikethrough);
        }

        let num = if let Some(index) = index {
            index
        } else {
            self.track_number as usize
        };

        let mut item = StyledString::styled(format!("{:02} ", num), style);
        item.append_styled(self.title.trim(), style.combine(Effect::Simple));
        item.append_plain(" ");

        let duration = ClockTime::from_seconds(self.duration as u64)
            .to_string()
            .as_str()[2..7]
            .to_string();

        item.append_styled(duration, style.combine(Effect::Dim));

        item
    }
}
