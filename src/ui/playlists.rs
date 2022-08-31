use crate::{
    qobuz::track::Track,
    ui::components::{Table, TableHeaders, TableRows, TableWidths},
};
use futures::executor;
use termion::event::{Key, MouseEvent};
use tui::{
    layout::{Constraint, Direction, Layout},
    text::Text,
    widgets::ListItem,
};

use crate::{
    player::Controls,
    qobuz::{client::Client, playlist::UserPlaylistsResult},
    state::app::AppState,
    ui::{
        components::{self, Item, List},
        Console, Screen,
    },
};

pub struct MyPlaylistsScreen<'m> {
    list: List<'m>,
    show_selected_list: bool,
    results: Option<UserPlaylistsResult>,
    open_playlist: Table,
    app_state: AppState,
    _controls: Controls,
    client: Client,
}

impl<'m> MyPlaylistsScreen<'m> {
    pub fn new(app_state: AppState, client: Client, _controls: Controls) -> Self {
        let list = List::new(None);
        let open_playlist = Table::new(None, None, None);

        let mut screen = MyPlaylistsScreen {
            show_selected_list: false,
            open_playlist,
            results: None,
            list,
            app_state,
            _controls,
            client,
        };
        screen.refresh_lists();

        screen
    }

    fn refresh_lists(&mut self) {
        if let Ok(my_lists) = executor::block_on(self.client.user_playlists()) {
            let list: Vec<String> = my_lists.into();
            let items = list
                .into_iter()
                .map(|i| ListItem::new(Text::raw(i)).into())
                .collect::<Vec<Item>>();

            self.list.set_items(items);
        }
    }
}

impl<'m> Screen for MyPlaylistsScreen<'m> {
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
                    .margin(0)
                    .split(f.size());

                components::player(f, layout[0], self.app_state.clone());

                if self.show_selected_list {
                    debug!("showing playlist");
                    components::table(f, &mut self.open_playlist, layout[1]);
                } else {
                    components::list(f, &mut self.list, layout[1]);
                }

                components::tabs(2, f, layout[2]);
            })
            .expect("failed to draw screen");
    }

    fn key_events(&mut self, key: Key) -> bool {
        match key {
            Key::Char(char) => match char {
                'r' => {
                    self.refresh_lists();
                    return true;
                }
                '\n' => {
                    if let (Some(selected), Some(r)) = (self.list.selected(), self.results.as_ref())
                    {
                        if let Some(list) = r.playlists.items.get(selected) {
                            if let Ok(mut playlist_info) =
                                executor::block_on(self.client.playlist(list.id.to_string()))
                            {
                                debug!("received playlist, adding to table");
                                playlist_info.reverse();

                                self.open_playlist.set_rows(playlist_info.rows());
                                self.open_playlist.set_header(Track::headers());
                                self.open_playlist.set_widths(Track::widths());
                                self.open_playlist.select(0);

                                self.show_selected_list = true;

                                return true;
                            }
                        }
                    }
                }
                _ => (),
            },
            Key::Esc => {
                self.show_selected_list = false;
                return true;
            }
            Key::Down => {
                self.list.next();
                return true;
            }
            Key::Up => {
                self.list.previous();
                return true;
            }
            _ => (),
        }

        false
    }

    fn mouse_events(&mut self, event: MouseEvent) -> bool {
        debug!("mouse event, {:?}", event);
        true
    }
}
