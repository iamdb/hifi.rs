use std::sync::Arc;

use termion::event::Key;
use tokio::sync::Mutex;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::{
    player::Controls,
    qobuz::AlbumSearchResults,
    state::app::AppState,
    switch_screen,
    ui::terminal::{components, AppKey, List, Screen, StateKey},
};

pub fn render<'t, B>(f: &mut Frame<B>, search_results: &'t mut List<'_>, app_state: AppState)
where
    B: Backend,
{
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .margin(0);

    let split_layout = layout.split(f.size());
    components::player(f, split_layout[0], app_state);
    components::track_list(f, search_results, split_layout[1]);
    components::tabs(1, f, split_layout[2]);
}

pub async fn key_events(
    key: Key,
    controls: Controls,
    search_results: Arc<Mutex<List<'_>>>,
    album_results: Option<AlbumSearchResults>,
    app_state: AppState,
) -> bool {
    let mut search_results = search_results.lock().await;

    match key {
        Key::Up => {
            search_results.previous();
            return true;
        }
        Key::Down => {
            search_results.next();
        }
        Key::Char('\n') => {
            if let Some(selected) = search_results.selected() {
                if let Some(album_results) = album_results {
                    if let Some(album) = album_results.albums.items.get(selected) {
                        controls.clear().await;
                        controls.play_album(album.clone()).await;
                        switch_screen!(app_state, Screen::NowPlaying);
                    };
                }
            }
        }
        _ => (),
    };

    false
}
