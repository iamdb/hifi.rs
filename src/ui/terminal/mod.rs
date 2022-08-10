pub mod player;

use self::player::TrackList;
use crate::{
    get_app,
    player::Controls,
    qobuz::{client::Client, AlbumSearchResults},
    state::{
        app::{AppKey, AppState, StateKey},
        Screen,
    },
    switch_screen,
    ui::terminal::{
        self,
        player::{player, Item},
    },
    REFRESH_RESOLUTION,
};
use flume::{Receiver, Sender};
use snafu::prelude::*;
use std::{char, io::Stdout, sync::Arc, thread, time::Duration};
use termion::{
    event::Key,
    input::{MouseTerminal, TermRead},
    raw::{IntoRawMode, RawTerminal},
    screen::AlternateScreen,
};
use tokio::{
    select,
    sync::{broadcast::Receiver as BroadcastReceiver, Mutex},
};
use tokio_stream::StreamExt;
use tui::{
    backend::{Backend, TermionBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::bar,
    text::{Span, Spans, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Tabs},
    Frame, Terminal,
};

pub struct Tui<'t> {
    rx: Receiver<Event>,
    tx: Sender<Event>,
    track_list: Arc<Mutex<TrackList<'t>>>,
    app_state: AppState,
    controls: Controls,
    no_tui: bool,
    terminal: Console,
    show_search: bool,
    search_query: Vec<char>,
    search_results: Arc<Mutex<TrackList<'t>>>,
    album_results: Option<AlbumSearchResults>,
}

type Console = Terminal<TermionBackend<AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>>>;

pub enum Event {
    Input(Key),
    Tick,
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Error getting stdout raw mode."))]
    RawMode,
}

impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Error::RawMode
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub fn new<'t>(app_state: AppState, controls: Controls, no_tui: bool) -> Result<Tui<'t>> {
    let stdout = std::io::stdout();
    let stdout = stdout.into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let terminal = Terminal::new(backend).unwrap();

    let (tx, rx) = flume::bounded(1);

    #[macro_export]
    macro_rules! switch_screen {
        ($app_state:expr, $screen:path) => {
            $app_state
                .app
                .insert::<String, Screen>(StateKey::App(AppKey::ActiveScreen), $screen);
        };
    }

    Ok(Tui {
        album_results: None,
        app_state,
        controls,
        no_tui,
        rx,
        search_query: Vec::new(),
        search_results: Arc::new(Mutex::new(TrackList::new(None))),
        show_search: false,
        terminal,
        track_list: Arc::new(Mutex::new(TrackList::new(None))),
        tx,
    })
}

impl<'t> Tui<'t> {
    pub async fn start(
        &mut self,
        client: Client,
        results: Option<AlbumSearchResults>,
    ) -> Result<()> {
        if !self.no_tui {
            let event_sender = self.tx.clone();
            let event_receiver = self.rx.clone();

            if let Some(results) = results {
                let items = results
                    .albums
                    .clone()
                    .item_list(self.terminal.size().unwrap().width as usize, false);

                let mut track_list = TrackList::new(Some(items));
                track_list.select(0);

                self.search_results = Arc::new(Mutex::new(track_list));
                self.search_results.lock().await.select(0);
                self.album_results = Some(results);
                self.app_state
                    .config
                    .insert::<String, Screen>(StateKey::App(AppKey::ActiveScreen), Screen::Search);
            }

            self.event_loop(
                client,
                event_sender,
                event_receiver,
                self.app_state.quitter(),
            )
            .await;
        } else {
            let mut quitter = self.app_state.quitter();

            let state = self.app_state.clone();
            ctrlc::set_handler(move || {
                state.quit();
                std::process::exit(0);
            })
            .expect("error setting ctrlc handler");

            loop {
                if let Ok(quit) = quitter.try_recv() {
                    if quit {
                        debug!("quitting");
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
            }
        };

        Ok(())
    }
    async fn event_loop(
        &mut self,
        client: Client,
        event_sender: Sender<Event>,
        event_receiver: Receiver<Event>,
        mut quitter: BroadcastReceiver<bool>,
    ) {
        let tx = event_sender.clone();
        thread::spawn(move || {
            let stdin = std::io::stdin();
            for key in stdin.keys().flatten() {
                debug!("key pressed {:?}", key);
                if let Err(err) = event_sender.send(Event::Input(key)) {
                    eprintln!("{}", err);
                    return;
                }
            }
        });

        thread::spawn(move || loop {
            if let Ok(quit) = quitter.try_recv() {
                if quit {
                    break;
                }
            }
            tx.send(Event::Tick).expect("failed to send tick");
            std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
        });

        let mut event_stream = event_receiver.stream();

        loop {
            select! {
                Some(event) = event_stream.next() => {
                    match event {
                       Event::Input(key) => {
                            if self.show_search {
                                let mut search_results = self.search_results.lock().await;

                                match key {
                                    Key::Char('\n') => {
                                        let query = self.search_query.drain(..).map(|q| q.to_string()).collect::<Vec<String>>().join("");

                                        if let Ok(results) = client.search_albums(query, Some(100)).await {
                                            self.album_results = Some(results.clone());

                                            let size = self.terminal.size().unwrap().width as usize;
                                            let items: Vec<Item> = results.albums.item_list(size, false);

                                            search_results.set_items(items);
                                            self.app_state.app.insert::<String, Screen>(StateKey::App(AppKey::ActiveScreen), Screen::Search);
                                        }

                                        self.show_search = false;
                                    }
                                    Key::Char(c) => {
                                        self.search_query.push(c)
                                    }
                                    Key::Backspace => {
                                        self.search_query.pop();
                                    }
                                    Key::Esc => {
                                        self.show_search = false;
                                    }
                                    _ => ()
                                }
                            } else {
                                match key {
                                    Key::Char('1') => {
                                        switch_screen!(self.app_state, Screen::NowPlaying);
                                    }
                                    Key::Char('2') =>  {
                                        switch_screen!(self.app_state, Screen::Search);
                                    }
                                    Key::Char('q') => {
                                        self.controls.stop().await;
                                        return;
                                    },
                                    Key::Char('/') => {
                                        self.show_search = true;
                                    }
                                    _ => {
                                        let app_tree = self.app_state.app.clone();
                                        if let Some(active_screen) = get_app!(AppKey::ActiveScreen, app_tree, Screen) {
                                            match active_screen {
                                                Screen::NowPlaying => {
                                                    player::key_events(event, self.controls.clone(), self.track_list.clone()).await;
                                                },
                                                Screen::Search => {
                                                    match event {
                                                       Event::Input(key) => {
                                                            match key {
                                                                Key::Up => {
                                                                    let mut search_results = self.search_results.lock().await;
                                                                    search_results.previous();
                                                                }
                                                                Key::Down => {
                                                                    let mut search_results = self.search_results.lock().await;
                                                                    search_results.next();
                                                                }
                                                                Key::Char('\n') => {
                                                                    let search_results = self.search_results.lock().await;
                                                                    if let Some(selected) = search_results.selected() {
                                                                        let album_results = self.album_results.clone();
                                                                        if let Some(results) = album_results {
                                                                            if let Some(album) = results.albums.items.get(selected) {
                                                                                self.controls.clear().await;
                                                                                self.controls.play_album(album.clone()).await;
                                                                                switch_screen!(self.app_state, Screen::NowPlaying);
                                                                            };
                                                                        }
                                                                    }
                                                                }
                                                                _ => ()
                                                            }
                                                        }
                                                        Event::Tick => ()
                                                    }
                                                    debug!("search key events");
                                                },
                                            }
                                        };
                                    }
                                }
                            }

                        }
                        Event::Tick => {
                            self.render().await;
                        }
                    }
                }
            }
        }
    }
    async fn render(&mut self) {
        let app_tree = self.app_state.app.clone();
        let screen = if let Some(saved_screen) = get_app!(AppKey::ActiveScreen, app_tree, Screen) {
            saved_screen
        } else {
            Screen::NowPlaying
        };

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Min(4),
                Constraint::Length(1),
            ])
            .margin(0);

        match screen {
            Screen::NowPlaying => {
                let mut list = self.track_list.lock().await;
                if let Some(items) = self
                    .app_state
                    .player
                    .item_list(self.terminal.size().unwrap().width as usize - 2)
                {
                    list.set_items(items);
                }

                self.terminal
                    .draw(|f| {
                        let split_layout = layout.split(f.size());

                        player(f, split_layout[0], self.app_state.clone());

                        if self.show_search {
                            search_popup(f, self.search_query.clone());
                        } else {
                            terminal::player::track_list(f, list.clone(), split_layout[1]);
                        }

                        tabs(0, f, split_layout[2]);
                    })
                    .expect("failed to draw terminal screen");
            }
            Screen::Search => {
                let list = self.search_results.lock().await;

                self.terminal
                    .draw(|f| {
                        let split_layout = layout.split(f.size());

                        player(f, split_layout[0], self.app_state.clone());

                        if self.show_search {
                            search_popup(f, self.search_query.clone());
                        } else {
                            terminal::player::track_list(f, list.clone(), split_layout[1]);
                        }

                        tabs(1, f, split_layout[2]);
                    })
                    .expect("failed to draw terminal screen");
            }
        }
    }
}

fn tabs<B>(num: usize, f: &mut Frame<B>, rect: Rect)
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

// #[macro_export]
// macro_rules! switch_screen {
//     ($app_state:expr, $screen:path) => {
//         $app_state
//             .app
//             .insert::<String, Screen>(StateKey::App(AppKey::ActiveScreen), $screen);
//     };
// }
