use crate::{
    player::Controls,
    state::app::AppState,
    ui::terminal::{
        components::{self, List},
        Screen,
    },
};
use termion::event::Key;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    Frame,
};

pub struct NowPlayingScreen {}

impl Screen for NowPlayingScreen {
    fn render<'t, B>(f: &mut Frame<B>, list: &'t mut List<'_>, app_state: AppState)
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

        if let Some(items) = app_state.player.item_list(f.size().width as usize - 2) {
            list.set_items(items);
        }

        let split_layout = layout.split(f.size());

        components::player(f, split_layout[0], app_state.clone());
        components::track_list(f, list, split_layout[1]);
        components::tabs(0, f, split_layout[2]);
    }
}

pub async fn key_events<'l>(key: Key, controls: Controls, track_list: &'l mut List<'_>) -> bool {
    match key {
        Key::Char(c) => match c {
            ' ' => {
                controls.play_pause().await;
                return true;
            }
            'N' => {
                controls.next().await;
                return true;
            }
            'P' => {
                controls.previous().await;
                return true;
            }
            '\n' => {
                if let Some(selection) = track_list.selected() {
                    debug!("playing selected track {}", selection);
                    controls.skip_to(selection).await;
                }

                return true;
            }
            _ => (),
        },
        Key::Down => {
            track_list.next();
            return true;
        }
        Key::Up => {
            track_list.previous();
            return true;
        }
        Key::Right => {
            controls.jump_forward().await;
            return true;
        }
        Key::Left => {
            controls.jump_backward().await;
            return true;
        }
        _ => (),
    }

    false
}
