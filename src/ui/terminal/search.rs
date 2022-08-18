use futures::executor;
use termion::event::{Key, MouseButton, MouseEvent};
use tui::layout::{Constraint, Direction, Layout};

use crate::{
    player::Controls,
    qobuz::{client::Client, AlbumSearchResults},
    state::app::AppState,
    switch_screen,
    ui::terminal::{
        components::{self, Table},
        AppKey, Console, Screen, StateKey,
    },
};

pub struct SearchScreen<'l> {
    client: Client,
    search_results: Table<'l>,
    app_state: AppState,
    album_results: Option<AlbumSearchResults>,
    controls: Controls,
    search_query: Vec<char>,
    enter_search: bool,
}

impl<'l> SearchScreen<'l> {
    pub fn new(
        app_state: AppState,
        controls: Controls,
        client: Client,
        album_results: Option<AlbumSearchResults>,
        query: Option<String>,
        screen_size: u16,
    ) -> SearchScreen<'l> {
        let mut enter_search = false;

        let search_results = if let Some(search_results) = album_results.clone() {
            let header = search_results
                .table_headers()
                .iter()
                .map(|h| h.to_string())
                .collect::<Vec<String>>();
            let mut table = Table::new(
                header,
                search_results.header_constraints(screen_size),
                Some(search_results.albums.row_list()),
            );
            table.select(0);
            switch_screen!(app_state, ActiveScreen::Search);

            table
        } else {
            enter_search = true;
            Table::new(Vec::new(), Vec::new(), None)
        };

        let search_query = if let Some(query) = query {
            query.chars().collect::<Vec<char>>()
        } else {
            Vec::new()
        };

        SearchScreen {
            album_results,
            app_state,
            client,
            controls,
            enter_search,
            search_query,
            search_results,
        }
    }
}

impl<'l> Screen for SearchScreen<'l> {
    fn render(&mut self, terminal: &mut Console) {
        terminal
            .draw(|f| {
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(6),
                        Constraint::Length(3),
                        Constraint::Min(4),
                        Constraint::Length(1),
                    ])
                    .margin(0)
                    .split(f.size());

                components::player(f, layout[0], self.app_state.clone());

                let text = String::from_iter(&self.search_query);
                components::text_box(f, text, Some("Search"), layout[1]);

                if self.enter_search {
                    f.set_cursor(
                        layout[1].x + 1 + self.search_query.len() as u16,
                        layout[1].y + 1,
                    );
                }

                components::table(f, &mut self.search_results, layout[2]);
                components::tabs(1, f, layout[3]);
            })
            .expect("failed to draw screen");
    }
    fn key_events(&mut self, key: Key) -> bool {
        match key {
            Key::Up => {
                self.search_results.previous();
                return true;
            }
            Key::Down => {
                self.search_results.next();
                return true;
            }
            Key::Backspace => {
                if self.enter_search {
                    self.search_query.pop();
                    return true;
                }
            }
            Key::Esc => {
                if self.enter_search {
                    self.enter_search = false;
                    return true;
                }
            }
            Key::Char(char) => match char {
                '\n' => {
                    if self.enter_search {
                        let query = String::from_iter(self.search_query.clone());
                        if let Ok(results) =
                            executor::block_on(self.client.search_albums(query, Some(100)))
                        {
                            self.album_results = Some(results.clone());
                            self.search_results.set_rows(results.albums.row_list());
                            self.search_results.select(0);
                            self.enter_search = false;
                        }
                    } else if let Some(selected) = self.search_results.selected() {
                        if let Some(album_results) = &self.album_results {
                            if let Some(album) = album_results.albums.items.get(selected) {
                                executor::block_on(self.controls.clear());
                                executor::block_on(self.controls.play_album(album.clone()));
                                switch_screen!(self.app_state, ActiveScreen::NowPlaying);
                                return true;
                            };
                        }
                    }
                }
                '/' => {
                    if !self.enter_search {
                        self.enter_search = true;
                        return true;
                    }
                }
                char => {
                    if self.enter_search {
                        self.search_query.push(char);
                        return true;
                    }
                }
            },
            _ => (),
        };

        false
    }
    fn mouse_events(&mut self, event: MouseEvent) -> bool {
        match event {
            MouseEvent::Press(button, _, y) => match button {
                MouseButton::Left => {
                    if y == 8 && !self.enter_search {
                        self.enter_search = true;
                        return true;
                    }
                }
                MouseButton::Right => {
                    debug!("right")
                }
                MouseButton::Middle => {
                    debug!("middle")
                }
                MouseButton::WheelUp => {
                    debug!("wheel up");
                    self.search_results.previous();
                    return true;
                }
                MouseButton::WheelDown => {
                    debug!("wheel down");
                    self.search_results.next();
                    return true;
                }
            },
            MouseEvent::Release(_, _) => {
                debug!("released")
            }
            MouseEvent::Hold(_, _) => debug!("held"),
        }

        false
    }
}
