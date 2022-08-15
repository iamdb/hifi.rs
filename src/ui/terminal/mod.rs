pub mod components;
pub mod nowplaying;
pub mod search;

use crate::{
    get_app,
    player::Controls,
    qobuz::{client::Client, AlbumSearchResults},
    state::{
        app::{AppKey, AppState, StateKey},
        Screen,
    },
    switch_screen,
    ui::terminal::components::List,
    REFRESH_RESOLUTION,
};
use flume::{Receiver, Sender};
use snafu::prelude::*;
use std::{char, io::Stdout, sync::Arc, thread, time::Duration};
use termion::{
    event::{Event as TermEvent, Key, MouseEvent},
    input::{MouseTerminal, TermRead},
    raw::{IntoRawMode, RawTerminal},
    screen::AlternateScreen,
};
use tokio::{select, sync::Mutex};
use tokio_stream::StreamExt;
use tui::{backend::TermionBackend, Terminal};

#[allow(unused)]
pub struct Tui<'t> {
    rx: Receiver<Event>,
    tx: Sender<Event>,
    track_list: Arc<Mutex<List<'t>>>,
    app_state: AppState,
    controls: Controls,
    no_tui: bool,
    terminal: Console,
    show_search: bool,
    search_query: Vec<char>,
    search_results: Arc<Mutex<List<'t>>>,
    album_results: Option<AlbumSearchResults>,
}

type Console = Terminal<TermionBackend<AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>>>;

pub enum Event {
    Key(Key),
    Mouse(MouseEvent),
    Unsupported(Vec<u8>),
    Tick,
}

impl From<TermEvent> for Event {
    fn from(e: TermEvent) -> Self {
        match e {
            TermEvent::Key(k) => Event::Key(k),
            TermEvent::Mouse(m) => Event::Mouse(m),
            TermEvent::Unsupported(u) => Event::Unsupported(u),
        }
    }
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

    let (tx, rx) = flume::bounded(2);

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
        search_results: Arc::new(Mutex::new(List::new(None))),
        show_search: false,
        terminal,
        track_list: Arc::new(Mutex::new(List::new(None))),
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
            if let Some(results) = results {
                let items = results
                    .albums
                    .clone()
                    .item_list(self.terminal.size().unwrap().width as usize, false);

                let mut track_list = List::new(Some(items));
                track_list.select(0);

                self.search_results = Arc::new(Mutex::new(track_list));
                self.search_results.lock().await.select(0);
                self.album_results = Some(results);
                switch_screen!(self.app_state, Screen::Search);
            }

            self.event_loop(client).await;
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
    async fn tick(&self) {
        if let Err(err) = self.tx.send_async(Event::Tick).await {
            error!("error sending tick: {}", err.to_string());
        }
    }
    async fn event_loop<'c>(&mut self, _client: Client) {
        let event_sender = self.tx.clone();
        let mut q = self.app_state.quitter();
        thread::spawn(move || {
            let stdin = std::io::stdin();
            for event in stdin.events().flatten() {
                if let Ok(quit) = q.try_recv() {
                    if quit {
                        return;
                    }
                }

                if let Err(err) = event_sender.send(event.into()) {
                    error!("error sending key event: {}", err.to_string());
                }
            }
        });

        let event_sender = self.tx.clone();
        let mut q = self.app_state.quitter();
        thread::spawn(move || loop {
            if let Ok(quit) = q.try_recv() {
                if quit {
                    break;
                }
            }
            if let Err(err) = event_sender.send(Event::Tick) {
                error!("error sending tick: {}", err.to_string());
            }
            std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
        });

        let event_receiver = self.rx.clone();
        let mut event_stream = event_receiver.stream();
        let mut quitter = self.app_state.quitter();

        loop {
            select! {
                Ok(quit) = quitter.recv() => {
                    if quit {
                        break;
                    }
                }
                Some(event) = event_stream.next() => {
                    match event {
                        Event::Tick => {
                            self.render().await;
                        },
                       Event::Key(key) => {
                            match key {
                                Key::Char('1') => {
                                    switch_screen!(self.app_state, Screen::NowPlaying);
                                    self.tick().await;
                                }
                                Key::Char('2') =>  {
                                    switch_screen!(self.app_state, Screen::Search);
                                    self.tick().await;
                                }
                                Key::Char('q') => {
                                    self.controls.stop().await;
                                    self.app_state.quit();
                                },
                                _ => {
                                    let app_tree = &self.app_state.app;
                                    if let Some(active_screen) = get_app!(AppKey::ActiveScreen, app_tree, Screen) {
                                        match active_screen {
                                            Screen::NowPlaying => {
                                                if nowplaying::key_events(key, self.controls.clone(), self.track_list.clone()).await {
                                                    self.tick().await;
                                                }
                                            },
                                            Screen::Search => {
                                                if search::key_events(key,self.controls.clone(),self.search_results.clone(),self.album_results.clone(),self.app_state.clone()).await {
                                                    self.tick().await;
                                                }
                                            }
                                        }

                                    };
                                }
                            }
                        },
                        Event::Mouse(m) => {
                            match m {
                                MouseEvent::Press(button, x, y) => println!("mouse press button {:?} at {}x{}", button, x, y),
                                MouseEvent::Release(x, y) => println!("mouse button released at {}x{}", x,y),
                                MouseEvent::Hold(x, y) => println!("mouse button held at {}x{}", x,y),
                            }
                        },
                        Event::Unsupported(_) => {
                            error!("unsupported input");
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

        match screen {
            Screen::NowPlaying => {
                let mut track_list = self.track_list.lock().await;
                self.terminal
                    .draw(|f| nowplaying::render(f, &mut track_list, self.app_state.clone()))
                    .expect("failed to draw terminal screen");
            }
            Screen::Search => {
                let mut search_results = self.search_results.lock().await;

                self.terminal
                    .draw(|f| search::render(f, &mut search_results, self.app_state.clone()))
                    .expect("failed to draw terminal screen");
            }
        }
    }
}
