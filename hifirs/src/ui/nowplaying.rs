use crate::{
    player::Controls,
    state::{
        app::{AppState, PlayerKey},
        TrackListValue,
    },
    ui::{
        components::{self, Row, Table, TableHeaders, TableRows, TableWidths},
        Console, Screen, StateKey,
    },
};
use futures::executor;
use qobuz_client::client::track::Track;
use termion::event::{Key, MouseEvent};
use tui::layout::{Constraint, Direction, Layout};

pub struct NowPlayingScreen {
    track_list: Table,
    app_state: AppState,
    controls: Controls,
}

impl NowPlayingScreen {
    pub fn new(app_state: AppState, controls: Controls) -> NowPlayingScreen {
        let track_list = Table::new(None, None, None);

        NowPlayingScreen {
            track_list,
            app_state,
            controls,
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

                components::player(f, split_layout[0], self.app_state.clone());

                let tree = self.app_state.player.clone();
                let mut title = "Now Playing".to_string();

                if let Some(tracklist) = get_player!(PlayerKey::Playlist, tree, TrackListValue) {
                    let mut rows = tracklist.rows();

                    if let Some(prev_playlist) =
                        get_player!(PlayerKey::PreviousPlaylist, tree, TrackListValue)
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
