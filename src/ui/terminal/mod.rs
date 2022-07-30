pub mod player;

use std::{io::Stdout, thread};

use flume::{Receiver, Sender};
use termion::{
    event::Key,
    input::{MouseTerminal, TermRead},
    raw::{IntoRawMode, RawTerminal},
    screen::AlternateScreen,
};
use tokio::{select, sync::broadcast::Receiver as BroadcastReceiver};
use tokio_stream::StreamExt;
use tui::{
    backend::TermionBackend,
    style::{Modifier, Style},
    widgets::ListItem,
    Terminal,
};

use crate::{
    player::{Player, Playlist},
    state::app::{AppKey, AppState, PlayerKey},
};

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
        let mut track_list = TrackList::new(None);

        loop {
            select! {
                Ok(state) = broadcast.recv() => {
                    if let Some(playlist) = state.player.get::<String, Playlist>(AppKey::Player(PlayerKey::Playlist)) {
                        let mut items = playlist
                            .into_iter()
                            .map(|t| {
                                let title = t.track.title;
                                ListItem::new(format!(" {:02}  {}", t.track.track_number, title)).style(Style::default())
                            })
                            .collect::<Vec<ListItem>>();

                        if let Some(prev_playlist) = state.player.get::<String, Playlist>(AppKey::Player(PlayerKey::PreviousPlaylist)) {
                            let mut prev_items = prev_playlist
                                .into_iter()
                                .map(|t| {
                                    let title = t.track.title;
                                    ListItem::new(format!(" {:02}  {}", t.track.track_number, title)).style(Style::default().add_modifier(Modifier::DIM))
                                })
                                .collect::<Vec<ListItem>>();

                            items.append(&mut prev_items);
                        }

                        track_list.set_items(items);

                        if track_list.selected().is_none() {
                            track_list.select(0);
                        }
                }
                    self.terminal.draw(|f| player::draw(f, state, track_list.clone())).unwrap();
                }
                Some(event) = event_stream.next() => {
                    if !player::key_events(event, player.clone(), track_list.clone()) {
                        break;
                    }
                }
            }
        }
    }
}
