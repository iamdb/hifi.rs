use crate::{
    sql::db::Database,
    state::{
        app::{AppKey, StateKey},
        ActiveScreen,
    },
    ui::components::{Table, TableHeaders, TableRows, TableWidths},
};
use futures::executor;
use qobuz_client::client::{
    api::Client,
    playlist::{Playlist, UserPlaylistsResult},
    track::Track,
};
use termion::event::Key;
use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Spans, Text},
    widgets::{Block, BorderType, Borders, ListItem, Tabs},
};

use crate::{
    player::Controls,
    ui::{
        components::{self, Item, List},
        Console, Screen,
    },
};

pub struct MyPlaylistsScreen<'m> {
    controls: Controls,
    db: Database,
    client: Client,
    mylist_results: Option<UserPlaylistsResult>,
    mylists: List<'m>,
    selected_playlist_result: Option<Playlist>,
    selected_playlist_table: Table,
    show_album_or_track_popup: bool,
    show_play_or_open_popup: bool,
    show_album_or_track_selection: usize,
    show_play_or_open_popup_selection: usize,
    show_selected_playlist: bool,
    screen_height: usize,
    screen_width: usize,
}

impl<'m> MyPlaylistsScreen<'m> {
    pub fn new(db: Database, client: Client, controls: Controls) -> Self {
        let mylists = List::new(None);
        let selected_playlist = Table::new(None, None, None);

        let mut screen = MyPlaylistsScreen {
            controls,
            show_album_or_track_selection: 0,
            screen_height: 0,
            screen_width: 0,
            db,
            client,
            mylist_results: None,
            mylists,
            selected_playlist_result: None,
            selected_playlist_table: selected_playlist,
            show_album_or_track_popup: false,
            show_play_or_open_popup: false,
            show_play_or_open_popup_selection: 0,
            show_selected_playlist: false,
        };
        screen.refresh_lists();

        screen
    }

    fn refresh_lists(&mut self) {
        if let Ok(my_lists) = executor::block_on(self.client.user_playlists()) {
            let list: Vec<String> = my_lists.clone().into();
            let items = list
                .into_iter()
                .map(|i| ListItem::new(Text::raw(i)).into())
                .collect::<Vec<Item>>();

            self.mylists.set_items(items);
            self.mylist_results = Some(my_lists);
        }
    }
}

impl<'m> Screen for MyPlaylistsScreen<'m> {
    fn render(&mut self, terminal: &mut Console) {
        terminal
            .draw(|f| {
                self.screen_height = f.size().height as usize;
                self.screen_width = f.size().width as usize;

                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(6),
                        Constraint::Min(4),
                        Constraint::Length(1),
                    ])
                    .margin(0)
                    .split(f.size());

                components::player(f, layout[0], self.db.clone());

                if self.show_selected_playlist {
                    components::table(
                        f,
                        &mut self.selected_playlist_table,
                        format!(
                            "Browsing playlist: {}",
                            &self.selected_playlist_result.clone().unwrap().name
                        )
                        .as_str(),
                        layout[1],
                    );
                } else {
                    components::list(f, &mut self.mylists, "Your Playlists", layout[1]);
                }

                if self.show_album_or_track_popup {
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Indexed(250)));

                    let titles = ["Album", "Track"].map(Spans::from).to_vec();

                    let tabs = Tabs::new(titles)
                        .block(block)
                        .style(Style::default().fg(Color::White))
                        .highlight_style(
                            Style::default()
                                .bg(Color::Indexed(81))
                                .fg(Color::Indexed(235))
                                .add_modifier(Modifier::BOLD),
                        )
                        .select(self.show_album_or_track_selection);

                    components::popup(f, tabs, 17, 3);
                }

                if self.show_play_or_open_popup {
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Indexed(250)));

                    let titles = ["Open", "Play"].map(Spans::from).to_vec();

                    let tabs = Tabs::new(titles)
                        .block(block)
                        .style(Style::default().fg(Color::White))
                        .highlight_style(
                            Style::default()
                                .bg(Color::Indexed(81))
                                .fg(Color::Indexed(235))
                                .add_modifier(Modifier::BOLD),
                        )
                        .select(self.show_play_or_open_popup_selection);

                    components::popup(f, tabs, 15, 3);
                }

                components::tabs(2, f, layout[2]);
            })
            .expect("failed to draw screen");
    }

    fn key_events(&mut self, key: Key) -> Option<()> {
        if self.show_album_or_track_popup || self.show_play_or_open_popup {
            match key {
                Key::Right | Key::Left | Key::Char('h') | Key::Char('l') => {
                    if self.show_album_or_track_popup {
                        if self.show_album_or_track_selection == 0 {
                            self.show_album_or_track_selection = 1;
                        } else if self.show_album_or_track_selection == 1 {
                            self.show_album_or_track_selection = 0;
                        }

                        Some(())
                    } else if self.show_play_or_open_popup {
                        if self.show_play_or_open_popup_selection == 0 {
                            self.show_play_or_open_popup_selection = 1;
                        } else if self.show_play_or_open_popup_selection == 1 {
                            self.show_play_or_open_popup_selection = 0;
                        }

                        Some(())
                    } else {
                        None
                    };
                }
                Key::Esc => {
                    if self.show_play_or_open_popup {
                        self.show_play_or_open_popup = false;
                        Some(())
                    } else {
                        None
                    };
                }
                Key::Char('\n') => {
                    if self.show_album_or_track_popup {
                        if let (Some(selected), Some(r)) = (
                            self.selected_playlist_table.selected(),
                            self.selected_playlist_result.as_ref(),
                        ) {
                            if let Some(tracks) = &r.tracks {
                                if let Some(track) = tracks.items.get(selected) {
                                    if self.show_album_or_track_selection == 0 {
                                        if let Some(album) = &track.album {
                                            if let Ok(album) = executor::block_on(
                                                self.client.album(album.id.clone()),
                                            ) {
                                                executor::block_on(self.controls.play_album(album));
                                                self.show_album_or_track_popup = false;

                                                executor::block_on(
                                                    self.db.insert::<String, ActiveScreen>(
                                                        StateKey::App(AppKey::ActiveScreen),
                                                        ActiveScreen::NowPlaying,
                                                    ),
                                                );
                                                Some(())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    } else if self.show_album_or_track_selection == 1 {
                                        executor::block_on(self.controls.play_track(track.clone()));
                                        self.show_album_or_track_popup = false;

                                        executor::block_on(self.db.insert::<String, ActiveScreen>(
                                            StateKey::App(AppKey::ActiveScreen),
                                            ActiveScreen::NowPlaying,
                                        ));

                                        Some(())
                                    } else {
                                        None
                                    };
                                }
                            }
                        }
                    } else if self.show_play_or_open_popup {
                        if self.show_play_or_open_popup_selection == 0 {
                            debug!("made selection");

                            if let (Some(selected), Some(r)) =
                                (self.mylists.selected(), self.mylist_results.as_ref())
                            {
                                debug!("selected item {}", selected);
                                if let Some(list) = r.playlists.items.get(selected) {
                                    debug!(
                                        "retrieved the playlist information {}-{}",
                                        list.name, list.id
                                    );
                                    debug!("fetching tracks for selected playlist");
                                    if let Ok(mut playlist_info) = executor::block_on(
                                        self.client.playlist(list.id.to_string()),
                                    ) {
                                        debug!("received playlist, adding to table");
                                        playlist_info.reverse();

                                        self.selected_playlist_table.set_rows(playlist_info.rows());
                                        self.selected_playlist_table.set_header(Track::headers());
                                        self.selected_playlist_table.set_widths(Track::widths());
                                        self.selected_playlist_table.select(0);

                                        self.selected_playlist_result = Some(playlist_info);

                                        self.show_selected_playlist = true;
                                        self.show_play_or_open_popup = false;

                                        Some(())
                                    } else {
                                        None
                                    };
                                }
                            }
                        } else if self.show_play_or_open_popup_selection == 1 {
                            if let (Some(results), Some(selected)) =
                                (&self.mylist_results, self.mylists.selected())
                            {
                                if let Some(playlist) = results.playlists.items.get(selected) {
                                    if let Ok(full_playlist) = executor::block_on(
                                        self.client.playlist(playlist.id.to_string()),
                                    ) {
                                        executor::block_on(
                                            self.controls.play_playlist(full_playlist),
                                        );

                                        self.show_play_or_open_popup = false;

                                        executor::block_on(self.db.insert::<String, ActiveScreen>(
                                            StateKey::App(AppKey::ActiveScreen),
                                            ActiveScreen::NowPlaying,
                                        ));

                                        Some(())
                                    } else {
                                        None
                                    };
                                }
                            }
                        }
                    }
                }
                _ => (),
            };
        }

        if self.show_selected_playlist {
            match key {
                Key::Down | Key::Char('j') => {
                    self.selected_playlist_table.next();
                    Some(())
                }
                Key::Up | Key::Char('k') => {
                    self.selected_playlist_table.previous();
                    Some(())
                }
                Key::Esc => {
                    self.show_selected_playlist = false;
                    Some(())
                }
                Key::Home => {
                    self.selected_playlist_table.home();
                    Some(())
                }
                Key::End => {
                    self.selected_playlist_table.end();
                    Some(())
                }
                Key::PageDown => {
                    let page_height = (self.screen_height / 2) as usize;

                    if let Some(selected) = self.selected_playlist_table.selected() {
                        if selected == 0 {
                            self.selected_playlist_table.select(page_height * 2);
                            Some(())
                        } else if selected + page_height > self.selected_playlist_table.len() - 1 {
                            self.selected_playlist_table
                                .select(self.selected_playlist_table.len() - 1);
                            Some(())
                        } else {
                            self.selected_playlist_table.select(selected + page_height);
                            Some(())
                        }
                    } else {
                        self.selected_playlist_table.select(page_height);
                        Some(())
                    }
                }
                Key::PageUp => {
                    let page_height = (self.screen_height / 2) as usize;

                    if let Some(selected) = self.selected_playlist_table.selected() {
                        if selected < page_height {
                            self.selected_playlist_table.select(0);
                            Some(())
                        } else {
                            self.selected_playlist_table.select(selected - page_height);
                            Some(())
                        }
                    } else {
                        self.selected_playlist_table.select(page_height);
                        Some(())
                    }
                }
                Key::Char('\n') => {
                    self.show_album_or_track_popup = true;
                    Some(())
                }
                _ => None,
            };
        }

        match key {
            Key::Down | Key::Char('j') => {
                self.mylists.next();
                Some(())
            }
            Key::Up | Key::Char('k') => {
                self.mylists.previous();
                Some(())
            }
            Key::Home => {
                self.mylists.select(0);
                Some(())
            }
            Key::End => {
                self.mylists.select(self.mylists.len() - 1);
                Some(())
            }
            Key::Char(char) => match char {
                'r' => {
                    self.refresh_lists();
                    Some(())
                }
                '\n' => {
                    self.show_play_or_open_popup = true;
                    Some(())
                }
                _ => None,
            },

            _ => None,
        }
    }
}
