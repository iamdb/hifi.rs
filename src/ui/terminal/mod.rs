pub mod player;

use self::player::TrackList;
use crate::{
    get_app,
    player::Controls,
    state::{
        app::{AppKey, AppState, StateKey},
        Screen,
    },
    ui::terminal::player::player,
    REFRESH_RESOLUTION,
};
use flume::{Receiver, Sender};
use snafu::prelude::*;
use std::{io::Stdout, sync::Arc, thread, time::Duration};
use termion::{
    event::Key,
    input::{MouseTerminal, TermRead},
    raw::{IntoRawMode, RawTerminal},
    screen::AlternateScreen,
};
use tokio::{select, sync::Mutex};
use tokio_stream::StreamExt;
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    symbols::DOT,
    text::Spans,
    widgets::{Block, Borders, Tabs},
    Terminal,
};

pub struct Tui {
    rx: Receiver<Event>,
    tx: Sender<Event>,
}

type Console = Terminal<TermionBackend<AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>>>;

pub enum Event {
    Input(Key),
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

pub fn new() -> Tui {
    let (tx, rx) = flume::bounded(1);

    Tui { rx, tx }
}

impl Tui {
    pub async fn start(&self, app_state: AppState, controls: Controls, no_tui: bool) -> Result<()> {
        let track_list = Arc::new(Mutex::new(TrackList::new(None)));

        if !no_tui {
            let stdout = std::io::stdout();
            let stdout = stdout.into_raw_mode()?;
            let stdout = MouseTerminal::from(stdout);
            let stdout = AlternateScreen::from(stdout);
            let backend = TermionBackend::new(stdout);
            let terminal = Terminal::new(backend).unwrap();

            let cloned_tracklist = track_list.clone();
            let cloned_state = app_state.clone();

            tokio::spawn(async move {
                render_loop(cloned_state, cloned_tracklist, terminal).await;
            });

            let event_sender = self.tx.clone();
            let event_receiver = self.rx.clone();
            event_loop(
                event_sender,
                event_receiver,
                track_list,
                controls.clone(),
                app_state,
            )
            .await;
        } else {
            let mut quitter = app_state.quitter();

            let state = app_state.clone();
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
        }

        Ok(())
    }
}

async fn event_loop(
    event_sender: Sender<Event>,
    event_receiver: Receiver<Event>,
    track_list: Arc<Mutex<TrackList<'static>>>,
    controls: Controls,
    app_state: AppState,
) {
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

    let mut event_stream = event_receiver.stream();
    let mut quitter = app_state.quitter();

    loop {
        select! {
            Ok(quit) = quitter.recv() => {
                if quit {
                    debug!("quitting");
                    break;
                }
            }
            Some(event) = event_stream.next() => {
                let Event::Input(key) = event;

                if key == Key::Char('1') {
                    app_state.app.insert::<String, Screen>(StateKey::App(AppKey::ActiveScreen), Screen::NowPlaying);
                } else if key == Key::Char('2') {
                    app_state.app.insert::<String, Screen>(StateKey::App(AppKey::ActiveScreen), Screen::Search);
                }

                player::key_events(event, controls.clone(), track_list.clone()).await;
            }
        }
    }
}
async fn render_loop(
    state: AppState,
    track_list: Arc<Mutex<TrackList<'_>>>,
    mut terminal: Console,
) {
    let mut quitter = state.quitter();

    loop {
        if let Ok(quit) = quitter.try_recv() {
            if quit {
                break;
            }
        }

        let app_tree = state.app.clone();
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
                Constraint::Length(4),
            ])
            .margin(0);

        match screen {
            Screen::NowPlaying => {
                let mut list = track_list.lock().await;
                if let Some(items) = state.player.item_list() {
                    list.set_items(items);
                }

                terminal
                    .draw(|f| {
                        let split_layout = layout.split(f.size());

                        player(f, split_layout[0], state.clone());

                        crate::ui::terminal::player::track_list(f, list.clone(), split_layout[1]);

                        let tabs = tabs(0);
                        f.render_widget(tabs, split_layout[2]);
                    })
                    .expect("failed to draw terminal screen");
            }
            Screen::Search => {
                terminal
                    .draw(|f| {
                        let split_layout = layout.split(f.size());

                        player(f, split_layout[0], state.clone());

                        let tabs = tabs(1);
                        f.render_widget(tabs, split_layout[2]);
                    })
                    .expect("failed to draw terminal screen");
            }
        }
        std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
    }
}

fn tabs<'a>(num: usize) -> Tabs<'a> {
    let titles = ["[1] Now Playing", "[2] Search"]
        .iter()
        .cloned()
        .map(Spans::from)
        .collect();

    Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow))
        .divider(DOT)
        .select(num)
}
