pub mod player;

use self::player::TrackList;
use crate::{player::Player, state::app::AppState, REFRESH_RESOLUTION};
use flume::{Receiver, Sender};
use parking_lot::Mutex;
use snafu::prelude::*;
use std::{io::Stdout, sync::Arc, thread, time::Duration};
use termion::{
    event::Key,
    input::{MouseTerminal, TermRead},
    raw::{IntoRawMode, RawTerminal},
    screen::AlternateScreen,
};
use tokio::select;
use tokio_stream::StreamExt;
use tui::{backend::TermionBackend, Terminal};

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
    pub async fn start(&self, state: AppState, player: Player, events_only: bool) -> Result<()> {
        let track_list = Arc::new(Mutex::new(TrackList::new(None)));
        let stdout = std::io::stdout();
        let stdout = stdout.into_raw_mode()?;
        let stdout = MouseTerminal::from(stdout);
        let stdout = AlternateScreen::from(stdout);
        let backend = TermionBackend::new(stdout);
        let terminal = Terminal::new(backend).unwrap();

        if !events_only {
            let cloned_tracklist = track_list.clone();
            tokio::spawn(async {
                render_loop(state, cloned_tracklist, terminal).await;
            });
        }

        let event_sender = self.tx.clone();
        let event_receiver = self.rx.clone();
        event_loop(event_sender, event_receiver, track_list, player).await;

        Ok(())
    }
}

async fn event_loop(
    event_sender: Sender<Event>,
    event_receiver: Receiver<Event>,
    track_list: Arc<Mutex<TrackList<'static>>>,
    player: Player,
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
    let mut quitter = player.app_state().quitter();

    loop {
        select! {
            Ok(quit) = quitter.recv() => {
                if quit {
                    debug!("quitting");
                    break;
                }
            }
            Some(event) = event_stream.next() => {
                player::key_events(event, player.clone(), track_list.clone());
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

        let mut track_list = track_list.lock();
        if let Some(items) = state.player.item_list() {
            track_list.set_items(items);
        }

        terminal
            .draw(|f| player::draw(f, state.clone(), track_list.clone()))
            .expect("failed to draw terminal screen");

        std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
    }
}
