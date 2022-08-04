pub mod player;

use std::{io::Stdout, thread, time::Duration};

use crate::{player::Player, state::app::AppState, REFRESH_RESOLUTION};
use flume::{Receiver, Sender};
use termion::{
    event::Key,
    input::{MouseTerminal, TermRead},
    raw::{IntoRawMode, RawTerminal},
    screen::AlternateScreen,
};
use tokio::select;
use tokio_stream::StreamExt;
use tui::{backend::TermionBackend, Terminal};

use self::player::TrackList;

pub struct Tui<'t> {
    rx: Receiver<Event>,
    tx: Sender<Event>,
    track_list: TrackList<'t>,
}

type Console = Terminal<TermionBackend<AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>>>;

pub enum Event {
    Input(Key),
}

pub fn new() -> Tui<'static> {
    let (tx, rx) = flume::bounded(1);

    Tui {
        rx,
        tx,
        track_list: TrackList::new(None),
    }
}

impl Tui<'static> {
    pub async fn start(&self, state: AppState, player: Player) {
        let stdout = std::io::stdout();
        let stdout = stdout.into_raw_mode().expect("Error getting raw mode");
        let stdout = MouseTerminal::from(stdout);
        let stdout = AlternateScreen::from(stdout);
        let backend = TermionBackend::new(stdout);
        let terminal = Terminal::new(backend).unwrap();

        let event_sender = self.tx.clone();
        let event_receiver = self.rx.clone();
        let track_list = self.track_list.clone();

        tokio::spawn(async {
            render_loop(state, track_list, terminal).await;
        });

        event_loop(
            event_sender,
            event_receiver,
            self.track_list.clone(),
            player,
        )
        .await;
    }
}

async fn event_loop(
    event_sender: Sender<Event>,
    event_receiver: Receiver<Event>,
    track_list: TrackList<'static>,
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
async fn render_loop(state: AppState, mut track_list: TrackList<'_>, mut terminal: Console) {
    let mut quitter = state.quitter();

    loop {
        if let Ok(quit) = quitter.try_recv() {
            if quit {
                break;
            }
        }

        if let Some(items) = state.player.clone().item_list() {
            track_list.set_items(items);

            if track_list.selected().is_none() {
                track_list.select(0);
            }
        }

        terminal
            .draw(|f| player::draw(f, state.clone(), track_list.clone()))
            .expect("failed to draw terminal screen");

        std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
    }
}
