use crate::state::{ClockValue, FloatValue, StatusValue};
use gstreamer::{ClockTime, State as GstState};
use qobuz_client::client::track::TrackListTrack;
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Gauge, Paragraph, Wrap},
    Frame,
};

pub(crate) fn progress<B>(
    position: ClockValue,
    duration: ClockValue,
    progress: FloatValue,
    is_buffering: bool,
    f: &mut Frame<B>,
    area: Rect,
) where
    B: Backend,
{
    if is_buffering {
        let text = "BUFFERING";
        let loading = Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Indexed(236)));

        f.render_widget(loading, area);
    } else if duration.inner_clocktime() > ClockTime::default() {
        let position = position.to_string().as_str()[2..7].to_string();
        let duration = duration.to_string().as_str()[2..7].to_string();
        let prog = if progress >= FloatValue(0.0) {
            progress
        } else {
            FloatValue(0.0)
        };

        let progress = Gauge::default()
            .label(format!("{position} / {duration}"))
            .use_unicode(true)
            .block(Block::default().style(Style::default().bg(Color::Indexed(236))))
            .gauge_style(
                Style::default()
                    .bg(Color::Indexed(235))
                    .fg(Color::Indexed(38))
                    .add_modifier(Modifier::BOLD),
            )
            .ratio(prog.into());
        f.render_widget(progress, area);
    } else {
        let text = "LOADING";
        let loading = Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Indexed(236)));

        f.render_widget(loading, area);
    }
}

pub(crate) fn current_track<B>(
    playlist_track: TrackListTrack,
    status: StatusValue,
    f: &mut Frame<B>,
    area: Rect,
) where
    B: Backend,
{
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
            Constraint::Length(9),
        ])
        .margin(0)
        .split(area);

    let spacer = Block::default().style(Style::default().bg(Color::Indexed(237)));
    f.render_widget(spacer.clone(), chunks[1]);
    f.render_widget(spacer, chunks[3]);

    let title_style = Style::default()
        .bg(Color::Indexed(24))
        .fg(Color::Indexed(81))
        .add_modifier(Modifier::BOLD);

    let mut current_track_text = vec![Spans::from(vec![Span::styled(
        playlist_track.track.title.clone(),
        title_style,
    )])];

    if let Some(performer) = playlist_track.track.performer {
        current_track_text.push(Spans::from(vec![Span::from(performer.name)]))
    }

    if playlist_track.track.title.len() <= chunks[2].width as usize {
        current_track_text.insert(0, Spans::from(""));
    }

    if let Some(album) = &playlist_track.album {
        let release_year =
            chrono::NaiveDate::parse_from_str(&album.release_date_original, "%Y-%m-%d")
                .unwrap()
                .format("%Y");
        current_track_text.push(Spans::from(format!("{} ({})", album.title, release_year)));
    }

    let current_track = Paragraph::new(current_track_text)
        .wrap(Wrap { trim: false })
        .block(Block::default().style(Style::default().bg(Color::Indexed(237))));

    let index = if playlist_track.album.is_some() {
        playlist_track.track.track_number as usize
    } else {
        playlist_track.index
    };

    let track_number_text = vec![
        Spans::from(""),
        Spans::from(format!("{index:02}")),
        Spans::from("of"),
        Spans::from(format!("{:02}", playlist_track.total)),
        Spans::from(""),
    ];

    let track_number = Paragraph::new(track_number_text)
        .block(
            Block::default().style(
                Style::default()
                    .bg(Color::Indexed(235))
                    .fg(Color::Indexed(31)),
            ),
        )
        .alignment(tui::layout::Alignment::Center);

    let mut resolution_text = vec![Spans::from("")];

    let current_state: String = match status.into() {
        GstState::Playing => '\u{25B6}'.to_string().to_uppercase(),
        GstState::Paused => '\u{23F8}'.to_string().to_uppercase(),
        GstState::Ready => "...".to_string().to_uppercase(),
        GstState::Null => "Null".to_string().to_uppercase(),
        _ => "".to_string(),
    };

    if !current_state.is_empty() {
        resolution_text.push(Spans::from(current_state));
    }

    if let Some(track_url) = playlist_track.track_url {
        resolution_text.push(Spans::from(format!("{}khz", track_url.sampling_rate)));
        resolution_text.push(Spans::from(format!("{}bit", track_url.bit_depth)));
        resolution_text.push(Spans::from(""));
    }

    if !resolution_text.is_empty() {
        let resolution = Paragraph::new(resolution_text)
            .alignment(Alignment::Center)
            .block(
                Block::default().style(
                    Style::default()
                        .bg(Color::Indexed(235))
                        .fg(Color::Indexed(31)),
                ),
            );

        f.render_widget(resolution, chunks[4]);
    }

    f.render_widget(track_number, chunks[0]);
    f.render_widget(current_track, chunks[2]);
}
