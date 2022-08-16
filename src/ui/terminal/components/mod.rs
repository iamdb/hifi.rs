use crate::{
    qobuz::PlaylistTrack,
    state::{
        app::{AppState, PlayerKey, StateKey},
        ClockValue, FloatValue, StatusValue,
    },
};
use gstreamer::{ClockTime, State as GstState};
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::bar,
    text::{Span, Spans, Text},
    widgets::{
        Block, BorderType, Borders, Clear, Gauge, List as TermList, ListItem, ListState, Paragraph,
        Tabs, Wrap,
    },
    Frame,
};

pub fn player<B>(f: &mut Frame<B>, rect: Rect, state: AppState)
where
    B: Backend,
{
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

pub fn text_box<B>(f: &mut Frame<B>, text: String, title: Option<&str>, area: Rect)
where
    B: Backend,
{
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Indexed(250)));

    if let Some(title) = title {
        block = block.title(title);
    }

    let p = Paragraph::new(text).block(block);

    f.render_widget(p, area);
}

pub fn list<'t, B>(f: &mut Frame<B>, list: &'t mut List<'_>, area: Rect)
where
    B: Backend,
{
    let layout = Layout::default()
        .margin(0)
        .constraints([Constraint::Min(1)])
        .split(area);

    let term_list = TermList::new(list.list_items())
        .highlight_style(
            Style::default()
                .fg(Color::Indexed(81))
                .bg(Color::Indexed(235)),
        )
        .highlight_symbol("");

    f.render_stateful_widget(term_list, layout[0], &mut list.state);
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
        let prog = if progress >= FloatValue(0.0) {
            progress
        } else {
            FloatValue(0.0)
        };

        let progress = Gauge::default()
            .label(format!("{} / {}", position, duration))
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
        .bg(Color::Indexed(24))
        .fg(Color::Indexed(81))
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

pub fn tabs<B>(num: usize, f: &mut Frame<B>, rect: Rect)
where
    B: Backend,
{
    let padding = (rect.width as usize / 2) - 4;

    let titles = ["Now Playing", "Search Results"]
        .iter()
        .cloned()
        .map(|t| {
            let text = format!("{:^padding$}", t);
            Spans::from(text)
        })
        .collect();

    let mut bar = Span::from(bar::FULL);
    bar.style = Style::default().fg(Color::Indexed(236));

    let tabs = Tabs::new(titles)
        .block(Block::default().style(Style::default().bg(Color::Indexed(235))))
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(81))
                .fg(Color::Indexed(235))
                .add_modifier(Modifier::BOLD),
        )
        .divider(bar)
        .select(num);

    f.render_widget(tabs, rect);
}
#[allow(unused)]
fn search_popup<B>(f: &mut Frame<B>, search_query: Vec<char>)
where
    B: Backend,
{
    let block = Block::default()
        .title("Enter query")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Indexed(250)));

    let p = Paragraph::new(Text::from(Spans::from(
        search_query
            .iter()
            .map(|c| Span::from(c.to_string()))
            .collect::<Vec<Span>>(),
    )))
    .block(block);

    let area = centered_rect(60, 10, f.size());

    f.render_widget(Clear, area);
    f.render_widget(p, area);
    f.set_cursor(area.x + 1 + search_query.len() as u16, area.y + 1);
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
/// https://github.com/fdehau/tui-rs/blob/master/examples/popup.rs
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
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
pub struct List<'t> {
    pub items: Vec<Item<'t>>,
    state: ListState,
}

impl<'t> List<'t> {
    pub fn new(items: Option<Vec<Item<'t>>>) -> List<'t> {
        if let Some(i) = items {
            List {
                items: i,
                state: ListState::default(),
            }
        } else {
            List {
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

    #[allow(unused)]
    pub fn select(&mut self, num: usize) {
        self.state.select(Some(num));
    }
}
