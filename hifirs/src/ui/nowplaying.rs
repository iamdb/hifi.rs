use crate::{
    player::Controls,
    sql::db::Database,
    state::{app::PlayerKey, TrackListValue},
    ui::{
        components::{self, Row, Table, TableHeaders, TableRows, TableWidths},
        Console, Screen, StateKey,
    },
};
use futures::executor;
use qobuz_client::client::track::Track;
use termion::event::Key;
use tui::layout::{Constraint, Direction, Layout};

pub struct NowPlayingScreen {
    track_list: Table,
    db: Database,
    controls: Controls,
    list_height: usize,
}

impl NowPlayingScreen {
    pub fn new(db: Database, controls: Controls) -> NowPlayingScreen {
        let track_list = Table::new(None, None, None);

        NowPlayingScreen {
            track_list,
            db,
            controls,
            list_height: 0,
        }
    }
}

impl Screen for NowPlayingScreen {
    fn render(&mut self, terminal: &mut Console) {
        terminal
            .draw(|f| {
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(6),
                        Constraint::Min(4),
                        Constraint::Length(1),
                    ])
                    .margin(0);

                let split_layout = layout.split(f.size());

                self.list_height = (split_layout[1].height - split_layout[1].y) as usize;

                components::player(f, split_layout[0], self.db.clone());

                let mut title = "Now Playing".to_string();

                if let Some(tracklist) = executor::block_on(
                    self.db
                        .get::<String, TrackListValue>(StateKey::Player(PlayerKey::Playlist)),
                ) {
                    let mut rows = tracklist.rows();

                    if let Some(prev_playlist) =
                        executor::block_on(self.db.get::<String, TrackListValue>(StateKey::Player(
                            PlayerKey::PreviousPlaylist,
                        )))
                    {
                        let prev_rows = prev_playlist.rows();
                        rows.append(
                            &mut prev_rows
                                .into_iter()
                                .map(|mut r| {
                                    r.set_dim(true);
                                    r
                                })
                                .collect::<Vec<Row>>(),
                        )
                    }

                    self.track_list.set_rows(rows);
                    self.track_list.set_header(Track::headers());
                    self.track_list.set_widths(Track::widths());

                    title = if let Some(album) = tracklist.get_album() {
                        format!("Album: {}", album.title)
                    } else if let Some(playlist) = tracklist.get_playlist() {
                        format!("Playlist: {}", playlist.name)
                    } else {
                        "Now Playing".to_string()
                    };
                }

                components::table(f, &mut self.track_list, title.as_str(), split_layout[1]);
                components::tabs(0, f, split_layout[2]);
            })
            .expect("failed to draw screen");
    }

    fn key_events(&mut self, key: Key) -> Option<()> {
        match key {
            Key::Down | Key::Char('j') => {
                self.track_list.next();
                Some(())
            }
            Key::Up | Key::Char('k') => {
                self.track_list.previous();
                Some(())
            }
            Key::Right | Key::Char('h') => {
                executor::block_on(self.controls.jump_forward());
                Some(())
            }
            Key::Left | Key::Char('l') => {
                executor::block_on(self.controls.jump_backward());
                Some(())
            }
            Key::Home => {
                self.track_list.home();
                Some(())
            }
            Key::End => {
                self.track_list.end();
                Some(())
            }
            Key::PageDown => {
                let page_height = (self.list_height / 2) as usize;

                if let Some(selected) = self.track_list.selected() {
                    if selected == 0 {
                        self.track_list.select(page_height * 2);
                        Some(())
                    } else if selected + page_height > self.track_list.len() - 1 {
                        self.track_list.select(self.track_list.len() - 1);
                        Some(())
                    } else {
                        self.track_list.select(selected + page_height);
                        Some(())
                    }
                } else {
                    self.track_list.select(page_height);
                    Some(())
                }
            }
            Key::PageUp => {
                let page_height = (self.list_height / 2) as usize;

                if let Some(selected) = self.track_list.selected() {
                    if selected < page_height {
                        self.track_list.select(0);
                        Some(())
                    } else {
                        self.track_list.select(selected - page_height);
                        Some(())
                    }
                } else {
                    self.track_list.select(page_height);
                    Some(())
                }
            }
            Key::Char(c) => match c {
                ' ' => {
                    executor::block_on(self.controls.play_pause());
                    Some(())
                }
                'N' => {
                    executor::block_on(self.controls.next());
                    Some(())
                }
                'P' => {
                    executor::block_on(self.controls.previous());
                    Some(())
                }
                '\n' => {
                    if let Some(selection) = self.track_list.selected() {
                        debug!("playing selected track {}", selection);
                        executor::block_on(self.controls.skip_to(selection));
                    }

                    Some(())
                }
                _ => None,
            },

            _ => None,
        };

        None
    }
}
