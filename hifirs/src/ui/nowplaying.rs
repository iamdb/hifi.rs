use crate::{
    player::Controls,
    state::app::{PlayerState, SkipDirection},
    ui::{
        components::{self, Table, TableHeaders, TableWidths},
        Console, Screen,
    },
};
use async_trait::async_trait;
use qobuz_client::client::track::Track;
use termion::event::Key;
use tui::layout::{Constraint, Direction, Layout};

pub struct NowPlayingScreen {
    track_list: Table,
    controls: Controls,
    list_height: usize,
    state: Option<PlayerState>,
}

impl NowPlayingScreen {
    pub fn new(controls: Controls) -> NowPlayingScreen {
        let track_list = Table::new(None, None, None);

        NowPlayingScreen {
            track_list,
            controls,
            list_height: 0,
            state: None,
        }
    }
}

#[async_trait]
impl Screen for NowPlayingScreen {
    fn render(&mut self, state: PlayerState, terminal: &mut Console) {
        self.state = Some(state.clone());

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

                self.list_height = if split_layout[1].height > split_layout[1].y {
                    (split_layout[1].height - split_layout[1].y) as usize
                } else {
                    1
                };

                components::player(f, split_layout[0], state.clone());

                let rows = state.rows();

                if !self.track_list.compare_rows(&rows) {
                    self.track_list.set_rows(rows);
                    self.track_list.set_header(Track::headers());
                    self.track_list.set_widths(Track::widths());
                }

                let title = if let Some(album) = state.album() {
                    format!("Album: {}", album.title)
                } else if let Some(playlist) = state.playlist() {
                    format!("Playlist: {}", playlist.name)
                } else {
                    "Now Playing".to_string()
                };

                components::table(f, &mut self.track_list, title.as_str(), split_layout[1]);
                components::tabs(0, f, split_layout[2]);
            })
            .expect("failed to draw screen");
    }

    async fn key_events(&mut self, key: Key) -> Option<()> {
        match key {
            Key::Down | Key::Char('j') => {
                self.track_list.next();
                Some(())
            }
            Key::Up | Key::Char('k') => {
                self.track_list.previous();
                Some(())
            }
            Key::Right | Key::Char('l') => {
                self.controls.jump_forward().await;
                Some(())
            }
            Key::Left | Key::Char('h') => {
                self.controls.jump_backward().await;
                Some(())
            }
            Key::Home => {
                self.track_list.home();
                Some(())
            }
            Key::End => {
                self.track_list.end();
                Some(())
            }
            Key::PageDown => {
                let page_height = self.list_height / 2;

                if let Some(selected) = self.track_list.selected() {
                    if selected == 0 {
                        self.track_list.select(page_height * 2);
                        Some(())
                    } else if selected + page_height > self.track_list.len() - 1 {
                        self.track_list.select(self.track_list.len() - 1);
                        Some(())
                    } else {
                        self.track_list.select(selected + page_height);
                        Some(())
                    }
                } else {
                    self.track_list.select(page_height);
                    Some(())
                }
            }
            Key::PageUp => {
                let page_height = self.list_height / 2;

                if let Some(selected) = self.track_list.selected() {
                    if selected < page_height {
                        self.track_list.select(0);
                        Some(())
                    } else {
                        self.track_list.select(selected - page_height);
                        Some(())
                    }
                } else {
                    self.track_list.select(page_height);
                    Some(())
                }
            }
            Key::Char(c) => match c {
                ' ' => {
                    self.controls.play_pause().await;
                    Some(())
                }
                'N' => {
                    self.controls.next().await;
                    Some(())
                }
                'P' => {
                    self.controls.previous().await;
                    Some(())
                }
                '\n' => {
                    if let Some(selection) = self.track_list.selected() {
                        if let Some(state) = &self.state {
                            if let Some(current_index) = state.current_track_index() {
                                let unplayed = state.unplayed_tracks().len();
                                let played = state.played_tracks().len();

                                let mut index = selection + 1;

                                if index <= unplayed {
                                    index += played;
                                } else {
                                    index = selection - unplayed;
                                }

                                debug!("playing selected track index: {index} selection: {selection} current: {current_index}");

                                let direction = if index > current_index {
                                    SkipDirection::Forward
                                } else {
                                    SkipDirection::Backward
                                };

                                self.controls.skip_to(index, direction).await;
                            }
                        }
                    }

                    Some(())
                }
                _ => None,
            },

            _ => None,
        }
    }
}
