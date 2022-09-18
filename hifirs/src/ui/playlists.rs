use crate::{
    switch_screen,
    ui::components::{Table, TableHeaders, TableRows, TableWidths},
};
use futures::executor;
use qobuz_client::client::{
    api::Client,
    playlist::{Playlist, UserPlaylistsResult},
    track::Track,
};
use termion::event::{Key, MouseEvent};
use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Spans, Text},
    widgets::{Block, BorderType, Borders, ListItem, Tabs},
};

use crate::{
    player::Controls,
    state::app::AppState,
    ui::{
        components::{self, Item, List},
        Console, Screen,
    },
};

pub struct MyPlaylistsScreen<'m> {
    controls: Controls,
    app_state: AppState,
    client: Client,
    mylist_results: Option<UserPlaylistsResult>,
    mylists: List<'m>,
    selected_playlist_result: Option<Playlist>,
    selected_playlist_table: Table,
    show_popup: bool,
    popup_selection: usize,
    show_selected_playlist: bool,
    screen_height: usize,
    screen_width: usize,
}

impl<'m> MyPlaylistsScreen<'m> {
    pub fn new(app_state: AppState, client: Client, controls: Controls) -> Self {
        let mylists = List::new(None);
        let selected_playlist = Table::new(None, None, None);

        let mut screen = MyPlaylistsScreen {
            controls,
            popup_selection: 0,
            screen_height: 0,
            screen_width: 0,
            app_state,
            client,
            mylist_results: None,
            mylists,
            selected_playlist_result: None,
            selected_playlist_table: selected_playlist,
            show_popup: false,
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

                components::player(f, layout[0], self.app_state.clone());

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

                if self.show_popup {
                    let block = Block::default()
                        .title("Play album or track?")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Indexed(250)));

                    let padding = ((f.size().width as f64 * 0.6) as usize / 2) - 3;

                    let titles = ["Album", "Track"]
                        .iter()
                        .cloned()
                        .map(|t| {
                            let text = format!("{:^padding$}", t);
                            Spans::from(text)
                        })
                        .collect();

                    let tabs = Tabs::new(titles)
                        .block(block)
                        .style(Style::default().fg(Color::White))
                        .highlight_style(
                            Style::default()
                                .bg(Color::Indexed(81))
                                .fg(Color::Indexed(235))
                                .add_modifier(Modifier::BOLD),
                        )
                        .select(self.popup_selection);

                    components::popup(f, tabs, 60, 10);
                }

                components::tabs(2, f, layout[2]);
            })
            .expect("failed to draw screen");
    }

    fn key_events(&mut self, key: Key) -> bool {
        if self.show_popup {
            match key {
                Key::Right | Key::Left | Key::Char('h') | Key::Char('l') => {
                    if self.show_popup {
                        if self.popup_selection == 0 {
                            self.popup_selection = 1;
                        } else if self.popup_selection == 1 {
                            self.popup_selection = 0;
                        }

                        return true;
                    }
                }
                Key::Char('\n') => {
                    if let (Some(selected), Some(r)) = (
                        self.selected_playlist_table.selected(),
                        self.selected_playlist_result.as_ref(),
                    ) {
                        if let Some(tracks) = &r.tracks {
                            if let Some(track) = tracks.items.get(selected) {
                                if self.popup_selection == 0 {
                                    if let Some(album) = &track.album {
                                        if let Ok(album) =
                                            executor::block_on(self.client.album(album.id.clone()))
                                        {
                                            executor::block_on(self.controls.play_album(album));
                                            self.show_popup = false;

                                            let app_state = self.app_state.clone();
                                            switch_screen!(app_state, ActiveScreen::NowPlaying);
                                        }
                                    }
                                } else if self.popup_selection == 1 {
                                    executor::block_on(self.controls.play_track(track.clone()));
                                    self.show_popup = false;

                                    let app_state = self.app_state.clone();
                                    switch_screen!(app_state, ActiveScreen::NowPlaying);
                                }
                            }
                        }
                    }

                    return true;
                }
                Key::Esc => {
                    self.show_popup = false;
                    return true;
                }
                _ => (),
            }

            return false;
        }

        if self.show_selected_playlist {
            match key {
                Key::Down | Key::Char('j') => {
                    self.selected_playlist_table.next();
                    return true;
                }
                Key::Up | Key::Char('k') => {
                    self.selected_playlist_table.previous();
                    return true;
                }
                Key::Esc => {
                    self.show_selected_playlist = false;
                    return true;
                }
                Key::Home => {
                    self.selected_playlist_table.select(0);
                    return true;
                }
                Key::End => {
                    self.selected_playlist_table
                        .select(self.selected_playlist_table.len() - 1);
                    return true;
                }
                Key::Char('\n') => {
                    self.show_popup = true;
                }
                _ => (),
            }

            return false;
        }

        match key {
            Key::Down | Key::Char('j') => {
                self.mylists.next();
                return true;
            }
            Key::Up | Key::Char('k') => {
                self.mylists.previous();
                return true;
            }
            Key::Home => {
                self.mylists.select(0);
                return true;
            }
            Key::End => {
                self.mylists.select(self.mylists.len() - 1);
                return true;
            }
            Key::Char(char) => match char {
                'r' => {
                    self.refresh_lists();
                    return true;
                }
                '\n' => {
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
                            if let Ok(mut playlist_info) =
                                executor::block_on(self.client.playlist(list.id.to_string()))
                            {
                                debug!("received playlist, adding to table");
                                playlist_info.reverse();

                                self.selected_playlist_table.set_rows(playlist_info.rows());
                                self.selected_playlist_table.set_header(Track::headers());
                                self.selected_playlist_table.set_widths(Track::widths());
                                self.selected_playlist_table.select(0);

                                self.selected_playlist_result = Some(playlist_info);

                                self.show_selected_playlist = true;

                                return true;
                            }
                        }
                    }
                }
                _ => (),
            },

            _ => (),
        }

        false
    }

    fn mouse_events(&mut self, event: MouseEvent) -> bool {
        match event {
            MouseEvent::Press(button, x, y) => match button {
                termion::event::MouseButton::Left => {
                    if y > 8 && (y as usize) < self.screen_height - 1 {
                        debug!("left mouse button, {x} {y}");
                        self.selected_playlist_table.select((y as usize) - 8);
                    }
                }
                termion::event::MouseButton::Right => todo!(),
                termion::event::MouseButton::Middle => todo!(),
                termion::event::MouseButton::WheelUp => todo!(),
                termion::event::MouseButton::WheelDown => todo!(),
            },
            MouseEvent::Release(x, y) => {
                debug!("button released {x} {y} {}", self.screen_height);
            }
            MouseEvent::Hold(_, _) => debug!("mouse hold"),
        }
        true
    }
}
