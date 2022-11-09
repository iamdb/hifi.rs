mod player;

use crate::{
    sql::db::Database,
    state::{
        app::{PlayerKey, StateKey},
        ClockValue, FloatValue, StatusValue,
    },
};
use futures::executor;
use qobuz_client::client::track::TrackListTrack;
use textwrap::fill;
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::bar,
    text::{Span, Spans},
    widgets::{
        Block, BorderType, Borders, Clear, List as TermList, ListItem, ListState, Paragraph,
        Row as TermRow, Table as TermTable, TableState, Tabs, Widget,
    },
    Frame,
};

pub fn player<B>(f: &mut Frame<B>, rect: Rect, db: Database)
where
    B: Backend,
{
    if let Some(track) =
        executor::block_on(db.get::<String, TrackListTrack>(StateKey::Player(PlayerKey::NextUp)))
    {
        if let Some(status) =
            executor::block_on(db.get::<String, StatusValue>(StateKey::Player(PlayerKey::Status)))
        {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Max(5), Constraint::Length(1)])
                .margin(0)
                .split(rect);

            player::current_track(track, status, f, layout[0]);

            if let (Some(position), Some(duration), Some(prog)) = (
                executor::block_on(
                    db.get::<String, ClockValue>(StateKey::Player(PlayerKey::Position)),
                ),
                executor::block_on(
                    db.get::<String, ClockValue>(StateKey::Player(PlayerKey::Duration)),
                ),
                executor::block_on(
                    db.get::<String, FloatValue>(StateKey::Player(PlayerKey::Progress)),
                ),
            ) {
                player::progress(position, duration, prog, f, layout[1]);
            }
        }
    } else {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .split(rect);

        let p = Paragraph::new("\nempty\n:(")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Indexed(81)))
            .block(
                Block::default()
                    .style(Style::default())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Indexed(234))),
            );

        f.render_widget(p, layout[0]);
    }
}

pub fn text_box<B>(f: &mut Frame<B>, text: String, title: Option<&str>, area: Rect)
where
    B: Backend,
{
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::Indexed(250)));

    if let Some(title) = title {
        block = block.title(title);
    }

    let p = Paragraph::new(text).block(block);

    f.render_widget(p, area);
}

#[allow(unused)]
pub fn list<'t, B>(f: &mut Frame<B>, list: &'t mut List<'_>, title: &str, area: Rect)
where
    B: Backend,
{
    let layout = Layout::default()
        .margin(0)
        .constraints([Constraint::Min(1)])
        .split(area);

    let term_list = TermList::new(list.list_items())
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .fg(Color::Indexed(81))
                .bg(Color::Indexed(235)),
        )
        .highlight_symbol("");

    f.render_stateful_widget(term_list, layout[0], &mut list.state);
}

pub fn table<'r, B>(f: &mut Frame<B>, table: &'r mut Table, title: &str, area: Rect)
where
    B: Backend,
{
    let (rows, widths, header) = table.term_table(f.size().width);

    let term_table = TermTable::new(rows)
        .header(
            TermRow::new(header).style(
                Style::default()
                    .bg(Color::Indexed(236))
                    .fg(Color::Indexed(81)),
            ),
        )
        .block(Block::default().borders(Borders::ALL).title(title))
        .widths(widths.as_slice())
        .style(Style::default().fg(Color::Indexed(250)))
        .highlight_style(
            Style::default()
                .fg(Color::Indexed(81))
                .bg(Color::Indexed(235)),
        );

    f.render_stateful_widget(term_table, area, &mut table.state.clone());
}

pub fn tabs<B>(num: usize, f: &mut Frame<B>, rect: Rect)
where
    B: Backend,
{
    let padding = (rect.width as usize / 3) - 2;

    let titles = ["Now Playing", "Search", "My Playlists"]
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
pub fn popup<B, W>(f: &mut Frame<B>, widget: W, width: u16, height: u16)
where
    B: Backend,
    W: Widget,
{
    let area = centered_rect(width, height, f.size());

    f.render_widget(Clear, area);
    f.render_widget(widget, area);
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
/// https://github.com/fdehau/tui-rs/blob/master/examples/popup.rs
fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let height_percent = 100 - ((height / r.height) * 100);

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(height_percent / 2),
                Constraint::Length(height),
                Constraint::Percentage(height_percent / 2),
            ]
            .as_ref(),
        )
        .split(r);

    let width_percent = 100. - ((width as f64 / r.width as f64) * 100.);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((width_percent as u16) / 2),
                Constraint::Length(width),
                Constraint::Percentage((width_percent as u16) / 2),
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

    pub fn select(&mut self, num: usize) {
        self.state.select(Some(num));
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    columns: Vec<String>,
    widths: Vec<ColumnWidth>,
    dim: bool,
}

impl Row {
    pub fn new(columns: Vec<String>, widths: Vec<ColumnWidth>) -> Row {
        Row {
            columns,
            widths,
            dim: false,
        }
    }

    pub fn set_dim(&mut self, dim: bool) {
        self.dim = dim;
    }

    pub fn insert_column(&mut self, index: usize, column: String) {
        self.columns.insert(index, column);
    }

    pub fn remove_column(&mut self, index: usize) {
        self.columns.remove(index);
    }

    pub fn term_row(&self, size: u16) -> TermRow<'_> {
        let column_widths = self
            .widths
            .iter()
            .map(|w| (size as f64 * (w.column_size as f64 * 0.01)).floor() as u16)
            .collect::<Vec<u16>>();

        let formatted = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let width = column_widths.get(i).unwrap();

                fill(c, *width as usize)
            })
            .collect::<Vec<String>>();

        let height = formatted
            .iter()
            .map(|f| {
                let count = f.matches('\n').count();

                if count == 0 {
                    1
                } else {
                    count + 1
                }
            })
            .max()
            .unwrap_or(1);

        let mut row_style = Style::default().fg(Color::White);

        if self.dim {
            row_style = Style::default().fg(Color::Indexed(244));
        }

        TermRow::new(formatted)
            .style(row_style)
            .height(height as u16)
    }
}

#[derive(Debug, Clone)]
pub struct ColumnWidth {
    /// Table column size in percent
    column_size: u16,
    constraint: Constraint,
}

impl ColumnWidth {
    /// Column sizes are in percent.
    pub fn new(column_size: u16) -> Self {
        ColumnWidth {
            column_size,
            constraint: Constraint::Percentage(column_size),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Table {
    rows: Vec<Row>,
    header: Vec<String>,
    state: TableState,
    widths: Vec<ColumnWidth>,
}

pub trait TableRows {
    fn rows(&self) -> Vec<Row>;
}

pub trait TableRow {
    fn row(&self) -> Row;
}

pub trait TableHeaders {
    fn headers() -> Vec<String>;
}

pub trait TableWidths {
    fn widths() -> Vec<ColumnWidth>;
}

impl Table {
    pub fn new(
        header: Option<Vec<String>>,
        items: Option<Vec<Row>>,
        widths: Option<Vec<ColumnWidth>>,
    ) -> Table {
        let mut state = TableState::default();
        state.select(Some(0));

        if let (Some(i), Some(header), Some(widths)) = (items, header, widths) {
            Table {
                rows: i,
                state,
                header,
                widths,
            }
        } else {
            Table {
                rows: Vec::new(),
                state,
                header: vec![],
                widths: vec![],
            }
        }
    }

    fn term_table(&self, size: u16) -> (Vec<TermRow>, Vec<Constraint>, Vec<String>) {
        let rows = self.term_rows(size);
        let widths = self
            .widths
            .iter()
            .map(|w| w.constraint)
            .collect::<Vec<Constraint>>();
        let header = self.header.clone();

        (rows, widths, header)
    }

    fn term_rows(&self, size: u16) -> Vec<TermRow> {
        self.rows
            .iter()
            .map(move |r| r.term_row(size))
            .collect::<Vec<TermRow>>()
    }

    pub fn set_header(&mut self, header: Vec<String>) {
        self.header = header;
    }

    pub fn set_rows(&mut self, rows: Vec<Row>) {
        self.rows = rows;
    }

    pub fn set_widths(&mut self, widths: Vec<ColumnWidth>) {
        self.widths = widths;
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if self.rows.is_empty() {
                    0
                } else if i >= self.rows.len() - 1 {
                    self.rows.len() - 1
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
                if self.rows.is_empty() || i == 0 {
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

    pub fn home(&mut self) {
        self.state.select(Some(0));
    }

    pub fn end(&mut self) {
        self.state.select(Some(self.rows.len() - 1));
    }

    pub fn at_end(&self) -> bool {
        if let Some(selected) = self.selected() {
            selected == self.rows.len() - 1
        } else {
            false
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
