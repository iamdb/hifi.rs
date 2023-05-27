use crate::{
    player::controls::Controls,
    qobuz::SearchResults,
    state::app::PlayerState,
    ui::{
        components::{self, ColumnWidth, Table, TableHeaders, TableRows, TableWidths},
        Console, Screen,
    },
};
use async_trait::async_trait;
use hifirs_qobuz_api::client::{
    album::{Album, AlbumSearchResults},
    api::Client,
};
use ratatui::layout::{Constraint, Direction, Layout};
use termion::event::Key;

pub struct SearchScreen {
    client: Client,
    results_table: Table,
    search_results: Option<SearchResults>,
    controls: Controls,
    search_query: Vec<char>,
    enter_search: bool,
    screen_width: u16,
    results_height: usize,
}

impl SearchScreen {
    pub fn new(
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
            client,
            controls,
            enter_search,
            search_query,
            results_table,
            screen_width,
            results_height: 0,
        }
    }

    async fn handle_selection(&mut self, results: &SearchResults, selected: usize) -> bool {
        match results {
            // An album has been selected, play it.
            SearchResults::Albums(results) => {
                if let Some(album) = results.albums.items.get(selected) {
                    self.controls.play_album(album.id.clone()).await;
                    return true;
                };
            }
            // An artist has been selected, load the albums into the list.
            SearchResults::Artists(results) => {
                if let Some(artist) = results.artists.items.get(selected) {
                    if let Ok(artist_info) = self
                        .client
                        .artist(artist.id.try_into().unwrap(), None)
                        .await
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

#[async_trait]
impl Screen for SearchScreen {
    fn render(&mut self, state: PlayerState, terminal: &mut Console) {
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

                self.results_height = if layout[2].height > layout[2].y {
                    (layout[2].height - layout[2].y) as usize
                } else {
                    1
                };

                components::player(f, layout[0], state);

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
    async fn key_events(&mut self, key: Key) -> Option<()> {
        match key {
            Key::Up | Key::Char('k') => {
                if self.enter_search {
                    self.search_query.push('k');
                    return Some(());
                } else {
                    self.results_table.previous();
                    return Some(());
                }
            }
            Key::Down | Key::Char('j') => {
                if self.enter_search {
                    self.search_query.push('j');
                    return Some(());
                } else {
                    self.results_table.next();
                    return Some(());
                }
            }
            Key::Right | Key::Char('l') => {
                if self.enter_search {
                    if key == Key::Char('l') {
                        self.search_query.push('l');
                    }
                } else {
                    self.controls.jump_forward().await;
                }
                return Some(());
            }
            Key::Left | Key::Char('h') => {
                if self.enter_search {
                    if key == Key::Char('h') {
                        self.search_query.push('h');
                    }
                } else {
                    self.controls.jump_backward().await;
                }
                return Some(());
            }
            Key::Backspace => {
                if self.enter_search {
                    self.search_query.pop();
                    return Some(());
                } else {
                    return None;
                }
            }
            Key::Esc => {
                if self.enter_search {
                    self.enter_search = false;
                    return Some(());
                } else {
                    return None;
                }
            }
            Key::Home => {
                self.results_table.home();
                return Some(());
            }
            Key::End => {
                self.results_table.end();
                return Some(());
            }
            Key::PageDown => {
                let page_height = self.results_height / 2;

                if let Some(selected) = self.results_table.selected() {
                    if selected == 0 {
                        self.results_table.select(page_height * 2);
                        return Some(());
                    } else if selected + page_height > self.results_table.len() - 1 {
                        self.results_table.select(self.results_table.len() - 1);
                        return Some(());
                    } else {
                        self.results_table.select(selected + page_height);
                        return Some(());
                    }
                } else {
                    self.results_table.select(page_height);
                    return Some(());
                }
            }
            Key::PageUp => {
                let page_height = self.results_height / 2;

                if let Some(selected) = self.results_table.selected() {
                    if selected < page_height {
                        self.results_table.select(0);
                        return Some(());
                    } else {
                        self.results_table.select(selected - page_height);
                        return Some(());
                    }
                } else {
                    self.results_table.select(page_height);
                    return Some(());
                }
            }
            Key::Char(char) => match char {
                ' ' => {
                    if !self.enter_search {
                        self.controls.play_pause().await;
                    } else {
                        self.search_query.push(' ');
                    }
                    return Some(());
                }
                'N' => {
                    if !self.enter_search {
                        self.controls.next().await;
                    } else {
                        self.search_query.push('N');
                    }
                    return Some(());
                }
                'P' => {
                    if !self.enter_search {
                        self.controls.previous().await;
                    } else {
                        self.search_query.push('P');
                    }
                    return Some(());
                }
                '\n' => {
                    if self.enter_search {
                        let query = String::from_iter(self.search_query.clone());
                        if let Ok(results) = self.client.search_artists(query, Some(100)).await {
                            let search_results = SearchResults::Artists(results);
                            self.search_results = Some(search_results.clone());
                            self.results_table = search_results.into();
                            self.results_table.select(0);
                            self.enter_search = false;

                            return Some(());
                        }
                    } else if let Some(selected) = self.results_table.selected() {
                        if let Some(results) = &self.search_results.clone() {
                            if self.handle_selection(results, selected).await {
                                return Some(());
                            } else {
                                return None;
                            };
                        }
                    }

                    return None;
                }
                '/' => {
                    if !self.enter_search {
                        self.enter_search = true;
                        return Some(());
                    } else {
                        return None;
                    }
                }
                char => {
                    if self.enter_search {
                        self.search_query.push(char);
                        return Some(());
                    } else {
                        return None;
                    }
                }
            },
            _ => return None,
        };
    }
}
