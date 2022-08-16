pub mod components;
pub mod nowplaying;
pub mod search;

use crate::{
    get_app,
    player::Controls,
    qobuz::{client::Client, AlbumSearchResults},
    state::{
        app::{AppKey, AppState, StateKey},
        ActiveScreen,
    },
    switch_screen,
    ui::terminal::{components::List, nowplaying::NowPlayingScreen, search::SearchScreen},
    REFRESH_RESOLUTION,
};
use flume::{Receiver, Sender};
use snafu::prelude::*;
use std::{cell::RefCell, collections::HashMap, io::Stdout, rc::Rc, thread, time::Duration};
use termion::{
    event::{Event as TermEvent, Key, MouseEvent},
    input::{MouseTerminal, TermRead},
    raw::{IntoRawMode, RawTerminal},
    screen::AlternateScreen,
};
use tokio::select;
use tokio_stream::StreamExt;
use tui::{backend::TermionBackend, Terminal};

pub trait Screen {
    fn render(&mut self, terminal: &mut Console);
    fn key_events(&mut self, key: Key) -> bool;
    fn mouse_events(&mut self, event: MouseEvent) -> bool;
}

#[allow(unused)]
pub struct Tui {
    rx: Receiver<Event>,
    tx: Sender<Event>,
    app_state: AppState,
    controls: Controls,
    terminal: Console,
    screens: HashMap<ActiveScreen, Rc<RefCell<dyn Screen>>>,
}

type Console = Terminal<TermionBackend<AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>>>;

/// An input event from a keyboard, mouse or internal timer.
#[derive(Debug, Clone)]
pub enum Event {
    /// Keyboard event
    Key(Key),
    /// Mouse button event
    Mouse(MouseEvent),
    /// Unsupported event
    Unsupported(Vec<u8>),
    /// Tick event (triggers frame render)
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

pub fn new(
    app_state: AppState,
    controls: Controls,
    client: Client,
    search_results: Option<AlbumSearchResults>,
    query: Option<String>,
) -> Result<Tui> {
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
            use $crate::state::ActiveScreen;

            $app_state
                .app
                .insert::<String, ActiveScreen>(StateKey::App(AppKey::ActiveScreen), $screen);
        };
    }

    let mut screens = HashMap::new();
    screens.insert(
        ActiveScreen::Search,
        Rc::new(RefCell::new(SearchScreen::new(
            app_state.clone(),
            controls.clone(),
            client,
            search_results,
            query,
        ))) as Rc<RefCell<dyn Screen>>,
    );
    screens.insert(
        ActiveScreen::NowPlaying,
        Rc::new(RefCell::new(NowPlayingScreen::new(
            app_state.clone(),
            controls.clone(),
            None,
        ))) as Rc<RefCell<dyn Screen>>,
    );

    Ok(Tui {
        app_state,
        controls,
        rx,
        terminal,
        tx,
        screens,
    })
}

impl Tui {
    pub async fn start(&mut self) -> Result<()> {
        self.event_loop().await;

        Ok(())
    }
    async fn tick(&self) {
        if let Err(err) = self.tx.send_async(Event::Tick).await {
            error!("error sending tick: {}", err.to_string());
        }
    }
    async fn render(&mut self) {
        let app_tree = self.app_state.app.clone();
        let screen =
            if let Some(saved_screen) = get_app!(AppKey::ActiveScreen, app_tree, ActiveScreen) {
                saved_screen
            } else {
                ActiveScreen::NowPlaying
            };

        match screen {
            ActiveScreen::NowPlaying => {
                if let Some(screen) = self.screens.get(&ActiveScreen::NowPlaying) {
                    screen.borrow_mut().render(&mut self.terminal);
                }
            }
            ActiveScreen::Search => {
                if let Some(screen) = self.screens.get(&ActiveScreen::Search) {
                    screen.borrow_mut().render(&mut self.terminal);
                }
            }
        }
    }
    async fn event_loop<'c>(&mut self) {
        // Watches stdin for input events and sends them to the
        // router for handling.
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

        // Sends a tick whose interval is defined by
        // REFRESH_RESOLUTION
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
                    self.handle_event(event).await
                }
            }
        }
    }
    async fn handle_event(&mut self, event: Event) {
        match event {
            Event::Tick => {
                self.render().await;
            }
            Event::Key(key) => match key {
                Key::Char('1') => {
                    switch_screen!(self.app_state, ActiveScreen::NowPlaying);
                    self.tick().await;
                }
                Key::Char('2') => {
                    switch_screen!(self.app_state, ActiveScreen::Search);
                    self.tick().await;
                }
                Key::Char('q') => {
                    self.controls.stop().await;
                    self.app_state.quit();
                }
                _ => {
                    let app_tree = &self.app_state.app;
                    if let Some(active_screen) =
                        get_app!(AppKey::ActiveScreen, app_tree, ActiveScreen)
                    {
                        match active_screen {
                            ActiveScreen::NowPlaying => {
                                if let Some(screen) = self.screens.get(&ActiveScreen::NowPlaying) {
                                    if screen.borrow_mut().key_events(key) {
                                        self.tick().await;
                                    }
                                }
                            }
                            ActiveScreen::Search => {
                                if let Some(screen) = self.screens.get(&ActiveScreen::Search) {
                                    if screen.borrow_mut().key_events(key) {
                                        self.tick().await;
                                    }
                                }
                            }
                        }
                    };
                }
            },
            Event::Mouse(m) => {
                let app_tree = &self.app_state.app;
                if let Some(active_screen) = get_app!(AppKey::ActiveScreen, app_tree, ActiveScreen)
                {
                    match active_screen {
                        ActiveScreen::NowPlaying => {
                            if let Some(screen) = self.screens.get(&ActiveScreen::NowPlaying) {
                                if screen.borrow_mut().mouse_events(m) {
                                    self.tick().await;
                                }
                            }
                        }
                        ActiveScreen::Search => {
                            if let Some(screen) = self.screens.get(&ActiveScreen::Search) {
                                if screen.borrow_mut().mouse_events(m) {
                                    self.tick().await;
                                }
                            }
                        }
                    }
                }
            }
            Event::Unsupported(_) => {
                error!("unsupported input");
            }
        }
    }
}
