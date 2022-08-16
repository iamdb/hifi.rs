use crate::{
    player::Controls,
    state::app::AppState,
    ui::terminal::{
        components::{self, Item, List},
        Console, Screen,
    },
};
use futures::executor;
use termion::event::{Key, MouseEvent};
use tui::layout::{Constraint, Direction, Layout};

pub struct NowPlayingScreen<'l> {
    track_list: List<'l>,
    app_state: AppState,
    controls: Controls,
}

impl<'l> NowPlayingScreen<'l> {
    pub fn new(
        app_state: AppState,
        controls: Controls,
        list_items: Option<Vec<Item<'_>>>,
    ) -> NowPlayingScreen {
        let track_list = if let Some(items) = list_items {
            List::new(Some(items))
        } else {
            List::new(None)
        };

        NowPlayingScreen {
            track_list,
            app_state,
            controls,
        }
    }
}

impl<'l> Screen for NowPlayingScreen<'l> {
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

                if let Some(items) = self.app_state.player.item_list(f.size().width as usize - 2) {
                    self.track_list.set_items(items);
                }

                let split_layout = layout.split(f.size());

                components::player(f, split_layout[0], self.app_state.clone());
                components::list(f, &mut self.track_list, split_layout[1]);
                components::tabs(0, f, split_layout[2]);
            })
            .expect("failed to draw screen");
    }

    fn key_events(&mut self, key: Key) -> bool {
        match key {
            Key::Char(c) => match c {
                ' ' => {
                    executor::block_on(self.controls.play_pause());
                    return true;
                }
                'N' => {
                    executor::block_on(self.controls.next());
                    return true;
                }
                'P' => {
                    executor::block_on(self.controls.previous());
                    return true;
                }
                '\n' => {
                    if let Some(selection) = self.track_list.selected() {
                        debug!("playing selected track {}", selection);
                        executor::block_on(self.controls.skip_to(selection));
                    }

                    return true;
                }
                _ => (),
            },
            Key::Down => {
                self.track_list.next();
                return true;
            }
            Key::Up => {
                self.track_list.previous();
                return true;
            }
            Key::Right => {
                executor::block_on(self.controls.jump_forward());
                return true;
            }
            Key::Left => {
                executor::block_on(self.controls.jump_backward());
                return true;
            }
            _ => (),
        }

        false
    }

    fn mouse_events(&mut self, _event: MouseEvent) -> bool {
        // match event {
        //     MouseEvent::Press(button, _, _) => match button {
        //         termion::event::MouseButton::Left => todo!(),
        //         termion::event::MouseButton::Right => todo!(),
        //         termion::event::MouseButton::Middle => todo!(),
        //         termion::event::MouseButton::WheelUp => todo!(),
        //         termion::event::MouseButton::WheelDown => todo!(),
        //     },
        //     MouseEvent::Release(_, _) => todo!(),
        //     MouseEvent::Hold(_, _) => todo!(),
        // };
        false
    }
}
