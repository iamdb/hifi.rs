use futures::executor;
use termion::event::Key;
use tui::layout::{Constraint, Direction, Layout};

use crate::{
    player::Controls,
    qobuz::AlbumSearchResults,
    state::app::AppState,
    switch_screen,
    ui::terminal::{components, AppKey, Console, List, Screen, StateKey},
};

pub struct SearchScreen<'l> {
    search_results: List<'l>,
    app_state: AppState,
    album_results: Option<AlbumSearchResults>,
    controls: Controls,
}

impl<'l> SearchScreen<'l> {
    pub fn new(
        app_state: AppState,
        controls: Controls,
        album_results: Option<AlbumSearchResults>,
    ) -> SearchScreen<'l> {
        let search_results = if let Some(search_results) = album_results.clone() {
            let mut list = List::new(Some(search_results.albums.item_list(100, false)));
            list.select(0);
            switch_screen!(app_state, ActiveScreen::Search);

            list
        } else {
            List::new(None)
        };

        SearchScreen {
            search_results,
            app_state,
            controls,
            album_results,
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
                        Constraint::Min(4),
                        Constraint::Length(1),
                    ])
                    .margin(0);

                let split_layout = layout.split(f.size());
                components::player(f, split_layout[0], self.app_state.clone());
                components::track_list(f, &mut self.search_results, split_layout[1]);
                components::tabs(1, f, split_layout[2]);
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
            Key::Char('\n') => {
                if let Some(selected) = self.search_results.selected() {
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
            _ => (),
        };

        false
    }
}
