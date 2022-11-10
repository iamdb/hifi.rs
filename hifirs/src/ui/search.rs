use crate::{
    player::Controls,
    qobuz::SearchResults,
    sql::db::Database,
    state::{
        app::{AppKey, StateKey},
        ActiveScreen,
    },
    ui::{
        components::{self, ColumnWidth, Table, TableHeaders, TableRows, TableWidths},
        Console, Screen,
    },
};
use futures::executor;
use qobuz_client::client::{
    album::{Album, AlbumSearchResults},
    api::Client,
};
use termion::event::Key;
use tui::layout::{Constraint, Direction, Layout};

pub struct SearchScreen {
    client: Client,
    results_table: Table,
    db: Database,
    search_results: Option<SearchResults>,
    controls: Controls,
    search_query: Vec<char>,
    enter_search: bool,
    screen_width: u16,
    results_height: usize,
}

impl SearchScreen {
    pub fn new(
        db: Database,
        controls: Controls,
        client: Client,
        search_results: Option<SearchResults>,
        query: Option<String>,
        screen_width: u16,
    ) -> SearchScreen {
        let enter_search = false;

        let results_table = if let Some(search_results) = search_results.clone() {
            let mut table: Table = search_results.into();
            table.select(0);

            table
        } else {
            Table::new(None, None, None)
        };

        let search_query = if let Some(query) = query {
            query.chars().collect::<Vec<char>>()
        } else {
            Vec::new()
        };

        SearchScreen {
            search_results,
            db,
            client,
            controls,
            enter_search,
            search_query,
            results_table,
            screen_width,
            results_height: 0,
        }
    }

    fn handle_selection(&mut self, results: &SearchResults, selected: usize) -> bool {
        match results {
            // An album has been selected, play it.
            SearchResults::Albums(results) => {
                if let Some(album) = results.albums.items.get(selected) {
                    executor::block_on(self.controls.play_album(album.clone()));
                    executor::block_on(self.db.insert::<String, ActiveScreen>(
                        StateKey::App(AppKey::ActiveScreen),
                        ActiveScreen::NowPlaying,
                    ));
                    return true;
                };
            }
            // An artist has been selected, load the albums into the list.
            SearchResults::Artists(results) => {
                if let Some(artist) = results.artists.items.get(selected) {
                    if let Ok(artist_info) =
                        executor::block_on(self.client.artist(artist.id.try_into().unwrap(), None))
                    {
                        if let Some(mut albums) = artist_info.albums {
                            albums.sort_by_date();
                            self.results_table.set_rows(albums.rows());
                            self.results_table.set_header(Album::headers());
                            self.results_table.set_widths(Album::widths());
                            self.results_table.select(0);

                            self.search_results = Some(SearchResults::Albums(AlbumSearchResults {
                                query: String::from_iter(&self.search_query),
                                albums,
                            }));
                        }
                    }
                };
            }
            _ => (),
        }

        false
    }
}

impl Screen for SearchScreen {
    fn render(&mut self, terminal: &mut Console) {
        terminal
            .draw(|f| {
                self.screen_width = f.size().width;

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

                self.results_height = (layout[2].height - layout[2].y) as usize;

                components::player(f, layout[0], self.db.clone());

                let text = String::from_iter(&self.search_query);
                components::text_box(f, text, Some("Search Artists"), layout[1]);

                if self.enter_search {
                    f.set_cursor(
                        layout[1].x + 1 + self.search_query.len() as u16,
                        layout[1].y + 1,
                    );
                }

                let widths = if let Some(results) = &self.search_results {
                    results.widths()
                } else {
                    vec![ColumnWidth::new(1)]
                };

                self.results_table.set_widths(widths);

                components::table(f, &mut self.results_table, "Search Results", layout[2]);
                components::tabs(1, f, layout[3]);
            })
            .expect("failed to draw screen");
    }
    fn key_events(&mut self, key: Key) -> bool {
        match key {
            Key::Up | Key::Char('k') => {
                self.results_table.previous();
                return true;
            }
            Key::Down | Key::Char('j') => {
                self.results_table.next();
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
            Key::Home => {
                self.results_table.home();
                return true;
            }
            Key::End => {
                self.results_table.end();
                return true;
            }
            Key::PageDown => {
                let page_height = (self.results_height / 2) as usize;

                if let Some(selected) = self.results_table.selected() {
                    if selected == 0 {
                        self.results_table.select(page_height * 2);
                        return true;
                    } else if selected + page_height > self.results_table.len() - 1 {
                        self.results_table.select(self.results_table.len() - 1);
                        return true;
                    } else {
                        self.results_table.select(selected + page_height);
                        return true;
                    }
                } else {
                    self.results_table.select(page_height);
                    return true;
                }
            }
            Key::PageUp => {
                let page_height = (self.results_height / 2) as usize;

                if let Some(selected) = self.results_table.selected() {
                    if selected < page_height {
                        self.results_table.select(0);
                        return true;
                    } else {
                        self.results_table.select(selected - page_height);
                        return true;
                    }
                } else {
                    self.results_table.select(page_height);
                    return true;
                }
            }
            Key::Char(char) => match char {
                '\n' => {
                    if self.enter_search {
                        let query = String::from_iter(self.search_query.clone());
                        if let Ok(results) =
                            executor::block_on(self.client.search_artists(query, Some(100)))
                        {
                            let search_results = SearchResults::Artists(results);
                            self.search_results = Some(search_results.clone());
                            self.results_table = search_results.into();
                            self.results_table.select(0);
                            self.enter_search = false;
                        }
                    } else if let Some(selected) = self.results_table.selected() {
                        if let Some(results) = &self.search_results.clone() {
                            return self.handle_selection(results, selected);
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
}
