pub mod components;
pub mod nowplaying;
pub mod playlists;
pub mod search;

use crate::{
    player::controls::Controls,
    qobuz::SearchResults,
    state::{
        app::{PlayerState, SafePlayerState},
        ActiveScreen,
    },
    switch_screen,
    ui::{nowplaying::NowPlayingScreen, playlists::MyPlaylistsScreen, search::SearchScreen},
    REFRESH_RESOLUTION,
};
use async_trait::async_trait;
use flume::{Receiver, Sender};
use hifirs_qobuz_api::client::api::Client;
use ratatui::{backend::TermionBackend, Terminal};
use snafu::prelude::*;
use std::{collections::HashMap, io::Stdout, sync::Arc, time::Duration};
use termion::{
    event::{Event as TermEvent, Key, MouseEvent},
    input::{MouseTerminal, TermRead},
    raw::{IntoRawMode, RawTerminal},
    screen::{AlternateScreen, IntoAlternateScreen},
};
use tokio::{select, sync::Mutex, time};
use tokio_stream::StreamExt;

#[macro_export]
macro_rules! switch_screen {
    ($state:expr, $screen:path) => {
        use $crate::state::ActiveScreen;

        $state.set_active_screen($screen);
    };
}

#[async_trait]
pub trait Screen {
    fn render(&mut self, state: PlayerState, terminal: &mut Console);
    async fn key_events(&mut self, key: Key) -> Option<()>;
}

pub struct Tui {
    rx: Receiver<Event>,
    tx: Sender<Event>,
    terminal: Console,
    screens: HashMap<ActiveScreen, Arc<Mutex<dyn Screen>>>,
    state: SafePlayerState,
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

pub async fn new(
    state: SafePlayerState,
    controls: Controls,
    client: Client,
    search_results: Option<SearchResults>,
    query: Option<String>,
) -> Result<Tui> {
    let stdout = std::io::stdout();
    let stdout = stdout.into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = stdout
        .into_alternate_screen()
        .expect("failed to convert to alternative screen");

    let backend = TermionBackend::new(stdout);
    let terminal = Terminal::new(backend).unwrap();

    let (tx, rx) = flume::unbounded();

    let mut screens = HashMap::new();
    screens.insert(
        ActiveScreen::Search,
        Arc::new(Mutex::new(SearchScreen::new(
            controls.clone(),
            client.clone(),
            search_results.clone(),
            query,
            terminal.size().unwrap().width,
        ))) as Arc<Mutex<dyn Screen>>,
    );
    screens.insert(
        ActiveScreen::NowPlaying,
        Arc::new(Mutex::new(NowPlayingScreen::new(controls.clone()))) as Arc<Mutex<dyn Screen>>,
    );

    screens.insert(
        ActiveScreen::Playlists,
        Arc::new(Mutex::new(
            MyPlaylistsScreen::new(client, controls.clone()).await,
        )) as Arc<Mutex<dyn Screen>>,
    );

    let tui = Tui {
        state: state.clone(),
        rx,
        terminal,
        tx,
        screens,
    };

    if let Some(results) = search_results {
        let mut state = state.write().await;

        match results {
            SearchResults::UserPlaylists(_) => {
                switch_screen!(state, ActiveScreen::Playlists);
            }
            _ => {
                switch_screen!(state, ActiveScreen::Search);
            }
        }
    }

    Ok(tui)
}

impl Tui {
    async fn tick(&self) {
        if let Err(err) = self.tx.send_async(Event::Tick).await {
            error!("error sending tick: {}", err.to_string());
        }
    }
    async fn render(&mut self) {
        let state = self.state.read().await.clone();
        let screen = state.active_screen();

        if let Some(screen) = self.screens.get(&screen) {
            screen.lock().await.render(state, &mut self.terminal);
        }
    }
    pub async fn event_loop<'c>(&mut self) -> Result<()> {
        // Watches stdin for input events and sends them to the
        // router for handling.
        let event_sender = self.tx.clone();

        let stdin_handle = tokio::spawn(async move {
            let stdin = std::io::stdin();
            for event in stdin.events().flatten() {
                if let Err(err) = event_sender.send_async(event.into()).await {
                    error!("error sending key event: {}", err.to_string());
                }
            }
        });

        // Sends a tick whose interval is defined by
        // REFRESH_RESOLUTION
        let event_sender = self.tx.clone();

        let tick_handle = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(REFRESH_RESOLUTION));

            loop {
                interval.tick().await;

                if let Err(err) = event_sender.send_async(Event::Tick).await {
                    debug!(
                        "error sending tick, app is probably just closing. err: {}",
                        err.to_string()
                    );
                    return;
                }
            }
        });

        let event_receiver = self.rx.clone();
        let mut event_stream = event_receiver.stream();
        let mut quitter = self.state.read().await.quitter();

        loop {
            select! {
                Ok(quit) = quitter.recv() => {
                    debug!("quitting input event stream");
                    if quit {
                        stdin_handle.abort();
                        tick_handle.abort();
                        return Ok(());
                    }
                }
                Some(event) = event_stream.next() => {
                    self.handle_event(event).await;
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
                Key::Char('\t') => {
                    let mut state = self.state.write().await;
                    let active_screen = state.active_screen();

                    match active_screen {
                        ActiveScreen::NowPlaying => {
                            switch_screen!(state, ActiveScreen::Search);
                            self.tick().await;
                        }
                        ActiveScreen::Search => {
                            switch_screen!(state, ActiveScreen::Playlists);
                            self.tick().await;
                        }
                        ActiveScreen::Playlists => {
                            switch_screen!(state, ActiveScreen::NowPlaying);
                            self.tick().await;
                        }
                    }
                }
                Key::Ctrl('c') | Key::Ctrl('q') => {
                    debug!("quitting ui handle event loop");
                    self.state.read().await.quit();
                }
                _ => {
                    if let Some(screen) = self.screens.get(&self.state.read().await.active_screen())
                    {
                        if screen.lock().await.key_events(key).await.is_some() {
                            self.tick().await;
                        }
                    };
                }
            },
            Event::Mouse(_) => {
                debug!("mouse not supported");
            }
            Event::Unsupported(_) => {
                debug!("unsupported input");
            }
        }
    }
}
