use std::sync::Arc;

use crate::{
    get_player,
    player::Controls,
    qobuz::PlaylistTrack,
    state::{
        app::{AppState, PlayerKey, StateKey},
        ClockValue, FloatValue, StatusValue,
    },
};
use gst::{ClockTime, State as GstState};
use gstreamer as gst;
use termion::event::Key;
use tokio::sync::Mutex;
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Gauge, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use super::Event;

pub fn player<B>(f: &mut Frame<B>, rect: Rect, state: AppState, empty_list: bool)
where
    B: Backend,
{
    if empty_list {
        let block = Block::default();
        let p = Paragraph::new("Select something to play").block(block);
        f.render_widget(p, rect);

        return;
    }

    let tree = state.player;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Max(5), Constraint::Length(1)])
        .margin(0)
        .split(rect);

    if let Some(track) = get_player!(PlayerKey::NextUp, tree, PlaylistTrack) {
        if let Some(status) = get_player!(PlayerKey::Status, tree, StatusValue) {
            current_track(track, status, f, layout[0]);
        }
    }

    if let (Some(position), Some(duration), Some(prog)) = (
        get_player!(PlayerKey::Position, tree, ClockValue),
        get_player!(PlayerKey::Duration, tree, ClockValue),
        get_player!(PlayerKey::Progress, tree, FloatValue),
    ) {
        progress(position, duration, prog, f, layout[1]);
    } else {
        f.render_widget(
            Block::default().style(Style::default().bg(Color::Indexed(236))),
            layout[1],
        )
    }
}

#[derive(Clone, Debug)]
pub struct Item<'i>(ListItem<'i>);

impl<'i> From<ListItem<'i>> for Item<'i> {
    fn from(item: ListItem<'i>) -> Self {
        Item(item)
    }
}

impl<'i> From<Item<'i>> for ListItem<'i> {
    fn from(item: Item<'i>) -> Self {
        item.0
    }
}

#[derive(Clone, Debug)]
pub struct TrackList<'t> {
    pub items: Vec<Item<'t>>,
    state: ListState,
}

impl<'t> TrackList<'t> {
    pub fn new(items: Option<Vec<Item<'t>>>) -> TrackList<'t> {
        if let Some(i) = items {
            TrackList {
                items: i,
                state: ListState::default(),
            }
        } else {
            TrackList {
                items: Vec::new(),
                state: ListState::default(),
            }
        }
    }

    pub fn list_items(&self) -> Vec<ListItem<'t>> {
        self.items
            .iter()
            .map(|item| item.clone().into())
            .collect::<Vec<ListItem<'_>>>()
    }

    pub fn set_items(&mut self, items: Vec<Item<'t>>) {
        if let Some(selected) = self.state.selected() {
            if selected > items.len() {
                self.state.select(Some(items.len()));
            } else {
                self.state.select(Some(selected))
            }
        } else {
            self.state.select(Some(0));
        }
        self.items = items;
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if self.items.is_empty() {
                    0
                } else if i >= self.items.len() - 1 {
                    self.items.len() - 1
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if self.items.is_empty() || i == 0 {
                    0
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn select(&mut self, num: usize) {
        self.state.select(Some(num));
    }
}

pub fn track_list<'a, B>(f: &mut Frame<B>, mut playlist: TrackList<'a>, area: Rect)
where
    B: Backend,
{
    let layout = Layout::default()
        .margin(1)
        .constraints([Constraint::Min(1)])
        .split(area);

    let list = List::new(playlist.list_items())
        .highlight_style(Style::default().fg(Color::Cyan))
        .highlight_symbol("");

    f.render_stateful_widget(list, layout[0], &mut playlist.state);
}

pub async fn key_events(event: Event, controls: Controls, track_list: Arc<Mutex<TrackList<'_>>>) {
    let Event::Input(key) = event;

    match key {
        Key::Char(c) => match c {
            'q' => controls.stop().await,
            ' ' => controls.play_pause().await,
            'N' => controls.next().await,
            'P' => controls.previous().await,
            '\n' => {
                let mut track_list = track_list.lock().await;

                if let Some(selection) = track_list.selected() {
                    debug!("playing selected track {}", selection);
                    controls.skip_to(selection).await;
                    track_list.select(0);
                }
            }
            _ => (),
        },
        Key::Down => {
            let mut track_list = track_list.lock().await;

            track_list.next();
        }
        Key::Up => {
            let mut track_list = track_list.lock().await;

            track_list.previous();
        }
        Key::Right => {
            controls.jump_forward().await;
        }
        Key::Left => {
            controls.jump_backward().await;
        }
        _ => (),
    }
}
fn progress<B>(
    position: ClockValue,
    duration: ClockValue,
    progress: FloatValue,
    f: &mut Frame<B>,
    area: Rect,
) where
    B: Backend,
{
    if duration.inner_clocktime() > ClockTime::default() {
        let position = position.to_string().as_str()[3..7].to_string();
        let duration = duration.to_string().as_str()[3..7].to_string();

        let progress = Gauge::default()
            .label(format!("{} / {}", position, duration))
            .use_unicode(true)
            .block(Block::default().style(Style::default().bg(Color::Indexed(236))))
            .gauge_style(
                Style::default()
                    .bg(Color::Indexed(236))
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .ratio(progress.into());
        f.render_widget(progress, area);
    } else {
        let loading = Paragraph::new("LOADING")
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Indexed(236)));

        f.render_widget(loading, area);
    }
}

fn current_track<B>(
    playlist_track: PlaylistTrack,
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
        .bg(Color::Cyan)
        .fg(Color::Indexed(236))
        .add_modifier(Modifier::BOLD);

    let mut current_track_text = vec![
        Spans::from(vec![Span::styled(
            playlist_track.track.title.clone(),
            title_style,
        )]),
        Spans::from(vec![Span::from(playlist_track.track.performer.name)]),
    ];

    if playlist_track.track.title.len() <= chunks[2].width as usize {
        current_track_text.insert(0, Spans::from(""));
    }

    let mut track_number_text = vec![
        Spans::from(""),
        Spans::from(format!("{:02}", playlist_track.track.track_number)),
    ];

    if let Some(album) = playlist_track.album {
        let release_year =
            chrono::NaiveDate::parse_from_str(&album.release_date_original, "%Y-%m-%d")
                .unwrap()
                .format("%Y");
        current_track_text.push(Spans::from(format!("{} ({})", album.title, release_year)));

        track_number_text.push(Spans::from("of"));
        track_number_text.push(Spans::from(format!("{:02}", album.tracks_count)));
        track_number_text.push(Spans::from(""));
    }

    let current_track = Paragraph::new(current_track_text)
        .wrap(Wrap { trim: false })
        .block(Block::default().style(Style::default().bg(Color::Indexed(237))));

    let track_number = Paragraph::new(track_number_text)
        .block(
            Block::default().style(
                Style::default()
                    .bg(Color::Indexed(236))
                    .fg(Color::Indexed(252)),
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
                        .bg(Color::Indexed(236))
                        .fg(Color::Indexed(252)),
                ),
            );

        f.render_widget(resolution, chunks[4]);
    }

    f.render_widget(track_number, chunks[0]);
    f.render_widget(current_track, chunks[2]);
}
