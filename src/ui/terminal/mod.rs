pub mod player;

use std::{io::Stdout, sync::Arc, thread};

use flume::{Receiver, Sender};
use parking_lot::RwLock;
use termion::{
    event::Key,
    input::{MouseTerminal, TermRead},
    raw::{IntoRawMode, RawTerminal},
    screen::AlternateScreen,
};
use tokio::{select, sync::broadcast::Receiver as BroadcastReceiver};
use tokio_stream::StreamExt;
use tui::{backend::TermionBackend, Terminal};

use crate::{player::Player, state::app::AppState};

use self::player::TrackList;

pub struct Tui {
    terminal: Console,
    rx: Receiver<Event>,
    tx: Sender<Event>,
}

type Console = Terminal<TermionBackend<AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>>>;

pub enum Event {
    Input(Key),
}

pub fn new() -> Tui {
    let stdout = std::io::stdout();
    let stdout = stdout.into_raw_mode().expect("Error getting raw mode");
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let terminal = Terminal::new(backend).unwrap();

    let (tx, rx) = flume::bounded(100);

    Tui { terminal, rx, tx }
}

impl Tui {
    pub async fn event_loop(&mut self, mut broadcast: BroadcastReceiver<AppState>, player: Player) {
        let sender = self.tx.clone();
        thread::spawn(move || {
            let stdin = std::io::stdin();
            for key in stdin.keys().flatten() {
                if let Err(err) = sender.send(Event::Input(key)) {
                    eprintln!("{}", err);
                    return;
                }
            }
        });

        let mut event_stream = self.rx.stream();
        let track_list = Arc::new(RwLock::new(TrackList::new(None)));
        let mut quitter = player.app_state().quitter();

        loop {
            select! {
                Ok(quit) = quitter.recv() => {
                    if quit {
                        debug!("quitting");
                        break;
                    }
                }
                Ok(state) = broadcast.recv() => {
                    self.terminal.draw(|f| player::draw(f, state, track_list.clone())).expect("failed to draw terminal screen");
                }
                Some(event) = event_stream.next() => {
                    player::key_events(event, player.clone(), track_list.clone());
                }
            }
        }
    }
}
