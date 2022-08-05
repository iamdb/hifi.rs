use std::sync::Arc;

use crate::{
    get_player,
    player::Player,
    qobuz::PlaylistTrack,
    state::{
        app::{AppKey, AppState, PlayerKey},
        ClockValue, FloatValue, StatusValue,
    },
};
use gst::{ClockTime, State as GstState};
use gstreamer as gst;
use parking_lot::Mutex;
use termion::event::Key;
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use super::Event;

pub fn draw<'a, B>(f: &mut Frame<B>, state: AppState, tracks: TrackList<'a>)
where
    B: Backend,
{
    let tree = state.player;

    let screen = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Min(4),
        ])
        .margin(0)
        .split(f.size());

    if let Some(track) = get_player!(PlayerKey::NextUp, tree, PlaylistTrack) {
        if let Some(status) = get_player!(PlayerKey::Status, tree, StatusValue) {
            current_track(track, status, f, screen[0]);
        }
    }

    track_list(f, tracks, screen[2]);

    if let (Some(position), Some(duration), Some(prog)) = (
        get_player!(PlayerKey::Position, tree, ClockValue),
        get_player!(PlayerKey::Duration, tree, ClockValue),
        get_player!(PlayerKey::Progress, tree, FloatValue),
    ) {
        if duration.inner_clocktime() > ClockTime::default() {
            progress(position, duration, prog, f, screen[1]);
        } else {
            let loading = Paragraph::new("LOADING")
                .alignment(Alignment::Center)
                .style(Style::default().bg(Color::Indexed(8)).fg(Color::Indexed(1)));

            f.render_widget(loading, screen[1]);
        }
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
    let position = position.to_string().as_str()[3..7].to_string();
    let duration = duration.to_string().as_str()[3..7].to_string();

    let progress = Gauge::default()
        .block(Block::default())
        .label(format!("{} / {}", position, duration))
        .use_unicode(true)
        .gauge_style(
            Style::default()
                .bg(Color::Indexed(4))
                .fg(Color::Indexed(0))
                .add_modifier(Modifier::BOLD),
        )
        .ratio(progress.into());

    f.render_widget(progress, area);
}

fn current_track<B>(
    playlist_track: PlaylistTrack,
    status: StatusValue,
    f: &mut Frame<B>,
    area: Rect,
) where
    B: Backend,
{
    let width: usize = area.width.try_into().unwrap();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(5),
            Constraint::Length(9),
        ])
        .margin(0)
        .split(area);

    let block = Block::default().style(Style::default());
    let title_style = Style::default()
        .bg(Color::Indexed(4))
        .fg(Color::Indexed(0))
        .add_modifier(Modifier::BOLD);

    let mut current_track_text = if playlist_track.track.title.len() > width - 8 {
        let line_1 = playlist_track.track.title.as_str()[0..width - 16]
            .trim()
            .to_string();
        let line_2 = playlist_track.track.title.as_str()
            [width - 16..playlist_track.track.title.len()]
            .trim()
            .to_string();

        vec![
            Spans::from(vec![
                Span::styled(" ", title_style),
                Span::styled(line_1, title_style),
                Span::styled(" ", title_style),
            ]),
            Spans::from(vec![
                Span::styled(" ", title_style),
                Span::styled(line_2, title_style),
                Span::styled(" ", title_style),
            ]),
            Spans::from(vec![
                Span::from(" "),
                Span::from(playlist_track.track.performer.name),
            ]),
        ]
    } else {
        vec![
            Spans::from(""),
            Spans::from(vec![
                Span::styled(" ", title_style),
                Span::styled(playlist_track.track.title, title_style),
                Span::styled(" ", title_style),
            ]),
            Spans::from(vec![
                Span::from(" "),
                Span::from(playlist_track.track.performer.name),
            ]),
        ]
    };

    let mut track_number_text = vec![
        Spans::from(""),
        Spans::from(format!(" {:02} ", playlist_track.track.track_number)),
    ];

    if let Some(album) = playlist_track.album {
        let release_year =
            chrono::NaiveDate::parse_from_str(&album.release_date_original, "%Y-%m-%d")
                .unwrap()
                .format("%Y");
        current_track_text.push(Spans::from(format!(" {} ({})", album.title, release_year)));

        track_number_text.push(Spans::from(" of "));
        track_number_text.push(Spans::from(format!(" {:02} ", album.tracks_count)));
        track_number_text.push(Spans::from(""));
    }

    let current_track = Paragraph::new(current_track_text).block(
        block
            .clone()
            .style(Style::default().fg(Color::Indexed(7)).bg(Color::Indexed(8))),
    );

    let track_number = Paragraph::new(track_number_text)
        .block(block.style(Style::default().fg(Color::Indexed(4)).bg(Color::Indexed(0))))
        .alignment(tui::layout::Alignment::Left)
        .wrap(Wrap { trim: false });

    let mut resolution_text = vec![];

    let current_state: String = match status.into() {
        GstState::Playing => '\u{25B6}'.to_string().to_uppercase(),
        GstState::Paused => '\u{23F8}'.to_string().to_uppercase(),
        GstState::Ready => "...".to_string().to_uppercase(),
        GstState::Null => " Null ".to_string().to_uppercase(),
        _ => "".to_string(),
    };

    if !current_state.is_empty() {
        resolution_text.push(Spans::from(""));
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
            .style(Style::default().fg(Color::Indexed(4)).bg(Color::Indexed(0)));

        f.render_widget(resolution, chunks[2]);
    }

    f.render_widget(track_number, chunks[0]);
    f.render_widget(current_track, chunks[1]);
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
}

fn track_list<'a, B>(f: &mut Frame<B>, mut playlist: TrackList<'a>, area: Rect)
where
    B: Backend,
{
    let list = List::new(playlist.list_items())
        .block(
            Block::default()
                .style(Style::default().bg(Color::Indexed(0)))
                .borders(Borders::ALL)
                .title("Track List \u{1F3BC}"),
        )
        .highlight_style(Style::default().fg(Color::Cyan))
        .highlight_symbol("•");

    f.render_stateful_widget(list, area, &mut playlist.state);
}

pub fn key_events(event: Event, player: Player, track_list: Arc<Mutex<TrackList<'_>>>) -> bool {
    let Event::Input(key) = event;
    let mut track_list = track_list.lock();

    match key {
        Key::Char(c) => match c {
            'q' => {
                player.app_state().send_quit();
                player.stop();
            }
            ' ' => {
                if player.is_playing() {
                    player.pause();
                } else if player.is_paused() {
                    player.play();
                }
            }
            'N' => {
                player.skip_forward(None).expect("failed to skip forward");
            }
            'P' => {
                player.skip_backward(None).expect("failed to skip backward");
            }
            '\n' => {
                if let Some(selection) = track_list.selected() {
                    debug!("playing selected track {}", selection);
                    player.skip_to(selection).expect("failed to skip to track");
                }
            }
            _ => (),
        },
        Key::Down => {
            track_list.next();
        }
        Key::Up => {
            track_list.previous();
        }
        Key::Right => {
            player.jump_forward();
        }
        Key::Left => {
            player.jump_backward();
        }
        _ => (),
    }

    true
}
