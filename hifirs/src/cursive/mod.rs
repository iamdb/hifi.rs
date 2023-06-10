use std::str::FromStr;

use crate::{
    player::{controls::Controls, notification::BroadcastReceiver, notification::Notification},
    state::TrackListType,
};
use cursive::{
    align::HAlign,
    direction::Orientation,
    event::{Event, Key},
    reexports::crossbeam_channel::Sender,
    theme::Effect,
    utils::{markup::StyledString, Counter},
    view::{Nameable, Resizable, Scrollable, SizeConstraint},
    views::{
        Button, Dialog, EditView, LinearLayout, NamedView, OnEventView, PaddedView, Panel,
        ProgressBar, RadioGroup, ResizedView, ScreensView, ScrollView, SelectView, TextView,
    },
    CbSink, Cursive, CursiveRunnable, With,
};
use futures::executor::block_on;
use gstreamer::{ClockTime, State as GstState};
use hifirs_qobuz_api::client::{api::Client, search_results::SearchAllResults};
use tokio::select;

pub struct CursiveUI<'c> {
    root: CursiveRunnable,
    controls: &'c Controls,
    client: Client,
}

impl<'c> CursiveUI<'c> {
    pub fn new(controls: &'c Controls, client: Client) -> Self {
        let siv = cursive::default();

        Self {
            root: siv,
            controls,
            client,
        }
    }

    pub fn player(&self) -> LinearLayout {
        let mut container = LinearLayout::new(Orientation::Vertical);
        let mut track_info = LinearLayout::new(Orientation::Horizontal);

        let meta = PaddedView::lrtb(
            1,
            1,
            0,
            0,
            LinearLayout::new(Orientation::Vertical)
                .child(
                    TextView::new("")
                        .style(Effect::Bold)
                        .with_name("current_track_title")
                        .scrollable()
                        .show_scrollbars(false)
                        .scroll_x(true),
                )
                .child(TextView::new("").with_name("artist_name"))
                .child(
                    TextView::new("")
                        .with_name("entity_title")
                        .scrollable()
                        .show_scrollbars(false)
                        .scroll_x(true),
                ),
        );

        let track_num = LinearLayout::new(Orientation::Vertical)
            .child(
                TextView::new("00")
                    .h_align(HAlign::Center)
                    .with_name("current_track_number"),
            )
            .child(TextView::new("of").h_align(HAlign::Center))
            .child(
                TextView::new("00")
                    .h_align(HAlign::Center)
                    .with_name("total_tracks"),
            )
            .fixed_width(2);

        let player_status = LinearLayout::new(Orientation::Vertical)
            .child(
                TextView::new("Null")
                    .h_align(HAlign::Center)
                    .with_name("player_status"),
            )
            .child(
                TextView::new("16 bits")
                    .h_align(HAlign::Right)
                    .with_name("bit_depth"),
            )
            .child(
                TextView::new("44.1 kHz")
                    .h_align(HAlign::Right)
                    .with_name("sample_rate"),
            )
            .fixed_width(8);

        let counter = Counter::new(0);
        let progress = ProgressBar::new()
            .with_value(counter)
            .with_label(|value, (_, max)| {
                let position =
                    ClockTime::from_seconds(value as u64).to_string().as_str()[2..7].to_string();
                let duration =
                    ClockTime::from_seconds(max as u64).to_string().as_str()[2..7].to_string();

                format!("{position} / {duration}")
            })
            .with_name("progress");

        track_info.add_child(track_num);
        track_info.add_child(meta.full_width());
        track_info.add_child(player_status);

        container.add_child(track_info);
        container.add_child(progress);

        let mut track_list: SelectView<usize> = SelectView::new();

        let c = self.controls.clone();
        track_list.set_on_submit(move |_s, item| {
            block_on(async { c.skip_to(*item).await });
        });

        LinearLayout::new(Orientation::Vertical)
            .child(
                Panel::new(container.resized(SizeConstraint::Full, SizeConstraint::AtMost(10)))
                    .title("player")
                    .with_name("player_panel"),
            )
            .child(Panel::new(
                track_list
                    .scrollable()
                    .scroll_y(true)
                    .with_name("current_track_list"),
            ))
    }

    fn nav(&self) -> ResizedView<LinearLayout> {
        let mut radio_group: RadioGroup<i32> = RadioGroup::new();

        radio_group.set_on_change(|s, item| {
            s.call_on_name(
                "screens",
                |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                    screens.set_active_screen(*item as usize);
                },
            );
        });

        LinearLayout::horizontal()
            .child(radio_group.button(0, "Now Playing").full_width())
            .child(radio_group.button(1, "My Playlists").full_width())
            .child(radio_group.button(2, "Search").full_width())
            .full_width()
    }

    pub fn global_events(&mut self) {
        self.root.clear_global_callbacks(Event::CtrlChar('c'));

        let c = self.controls.clone();
        self.root.set_on_pre_event(Event::CtrlChar('c'), move |s| {
            let c = c.clone();

            let dialog = Dialog::text("Do you want to quit?")
                .button("Yes", move |s: &mut Cursive| {
                    c.quit_blocking();
                    s.quit();
                })
                .dismiss_button("No");

            s.add_layer(dialog);
        });

        self.root.add_global_callback('1', move |s| {
            s.call_on_name(
                "screens",
                |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                    screens.set_active_screen(0);
                },
            );
        });
        self.root.add_global_callback('2', move |s| {
            s.call_on_name(
                "screens",
                |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                    screens.set_active_screen(1);
                },
            );
        });

        self.root.add_global_callback('3', move |s| {
            s.call_on_name(
                "screens",
                |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                    screens.set_active_screen(2);
                },
            );
        });

        let c = self.controls.clone();
        self.root.add_global_callback(' ', move |_| {
            block_on(async { c.play_pause().await });
        });

        let c = self.controls.clone();
        self.root.add_global_callback('N', move |_| {
            block_on(async { c.next().await });
        });

        let c = self.controls.clone();
        self.root.add_global_callback('P', move |_| {
            block_on(async { c.previous().await });
        });

        let c = self.controls.clone();
        self.root.add_global_callback('l', move |_| {
            block_on(async { c.jump_forward().await });
        });

        let c = self.controls.clone();
        self.root.add_global_callback('h', move |_| {
            block_on(async { c.jump_backward().await });
        });
    }

    pub async fn my_playlists(&self) -> LinearLayout {
        let mut list_layout = LinearLayout::new(Orientation::Vertical);

        let mut user_playlists = SelectView::new().popup();
        user_playlists.add_item("Select Playlist", 0);
        let playlist_items: SelectView<(i32, Option<String>)> = SelectView::new();

        if let Ok(my_playlists) = self.client.user_playlists().await {
            my_playlists.playlists.items.iter().for_each(|p| {
                user_playlists.add_item(p.name.clone(), p.id);
            });
        }

        let c = self.controls.to_owned();
        let play = move |s: &mut Cursive| {
            if let Some(view) = s.find_name::<SelectView<i64>>("user_playlists") {
                let c = c.to_owned();
                if let Some(id) = view.selection() {
                    block_on(async move { c.play_playlist(*id).await });
                }
            }
        };

        let play_button = Button::new("play", play)
            .disabled()
            .with_name("play_button");

        let c = self.controls.clone();
        let client = self.client.clone();
        user_playlists.set_on_submit(move |s: &mut Cursive, item: &i64| {
            if item == &0 {
                s.call_on_name(
                    "playlist_items",
                    |view: &mut SelectView<(i32, Option<String>)>| {
                        view.clear();
                    },
                );

                s.call_on_name("play_button", |button: &mut Button| {
                    button.disable();
                });

                return;
            }

            let c = c.clone();
            let client = client.clone();

            submit_playlist(s, *item, client, c);

            s.call_on_name("play_button", |button: &mut Button| {
                button.enable();
            });
        });

        list_layout.add_child(
            Panel::new(
                LinearLayout::horizontal()
                    .child(
                        user_playlists
                            .with_name("user_playlists")
                            .scrollable()
                            .scroll_y(true)
                            .full_width(),
                    )
                    .child(play_button),
            )
            .title("my playlists"),
        );
        list_layout.add_child(
            Panel::new(
                playlist_items
                    .with_name("playlist_items")
                    .scrollable()
                    .scroll_y(true),
            )
            .full_height(),
        );

        list_layout
    }

    fn search(&self) -> LinearLayout {
        let mut layout = LinearLayout::new(Orientation::Vertical);

        let c = self.controls.to_owned();
        let client = self.client.to_owned();

        let on_submit = move |s: &mut Cursive, item: &String| {
            let item = item.clone();

            s.call_on_name(
                "results_scroll",
                |scroll: &mut ScrollView<NamedView<SelectView<String>>>| {
                    scroll.scroll_to_top();
                },
            );

            if let Some(mut results) = s.find_name::<SelectView<String>>("search_results") {
                results.clear();

                if let Some(data) = s.user_data::<SearchAllResults>() {
                    match item.as_str() {
                        "Albums" => {
                            for a in &data.albums.items {
                                if !a.streamable {
                                    continue;
                                }

                                results.add_item(a.list_item(), a.id.clone());
                            }

                            let c = c.to_owned();
                            results.set_on_submit(move |_s: &mut Cursive, item: &String| {
                                block_on(async { c.play_album(item.clone()).await })
                            });
                        }
                        "Artists" => {
                            for a in &data.artists.items {
                                results.add_item(a.name.clone(), a.id.to_string());
                            }

                            let client = client.to_owned();
                            let c = c.to_owned();
                            results.set_on_submit(move |s: &mut Cursive, item: &String| {
                                let client = client.to_owned();
                                let c = c.to_owned();

                                submit_artist(
                                    s,
                                    item.parse::<i32>().expect("failed to parse string"),
                                    client,
                                    c,
                                );
                            });
                        }
                        "Tracks" => {
                            for t in &data.tracks.items {
                                if !t.streamable {
                                    continue;
                                }

                                results.add_item(t.list_item(), t.id.to_string())
                            }

                            let c = c.to_owned();
                            results.set_on_submit(move |s: &mut Cursive, item: &String| {
                                let c = c.to_owned();
                                submit_track(
                                    s,
                                    (item.parse::<i32>().expect("failed to parse string"), None),
                                    c,
                                );
                            });
                        }
                        "Playlists" => {
                            for p in &data.playlists.items {
                                results.add_item(p.name.clone(), p.id.to_string())
                            }

                            let c = c.to_owned();
                            let client = client.to_owned();
                            results.set_on_submit(move |s: &mut Cursive, item: &String| {
                                let c = c.to_owned();
                                let client = client.to_owned();
                                submit_playlist(
                                    s,
                                    item.parse::<i64>().expect("failed to parse string"),
                                    client,
                                    c,
                                );
                            });
                        }
                        _ => {}
                    }
                }
            }
        };

        let search_type = SelectView::new()
            .item_str("Albums")
            .item_str("Artists")
            .item_str("Tracks")
            .item_str("Playlists")
            .on_submit(on_submit.clone())
            .popup()
            .with_name("search_type")
            .wrap_with(Panel::new);

        let c = self.client.clone();
        let search_form = EditView::new()
            .on_submit_mut(move |s, item| {
                if let Ok(results) = block_on(async { c.search_all(item.to_string()).await }) {
                    debug!("saving search results to user data");
                    s.set_user_data(results);

                    if let Some(view) = s.find_name::<SelectView>("search_type") {
                        if let Some(value) = view.selection() {
                            on_submit(s, &value.to_string());
                            s.focus_name("search_results")
                                .expect("failed to focus on search results");
                        }
                    }
                }
            })
            .wrap_with(Panel::new);

        let search_results: NamedView<SelectView<String>> =
            SelectView::new().with_name("search_results");

        layout.add_child(search_form.title("search"));
        layout.add_child(search_type);
        layout.add_child(
            Panel::new(
                search_results
                    .scrollable()
                    .scroll_y(true)
                    .with_name("results_scroll"),
            )
            .with_name("results_panel")
            .full_height(),
        );

        layout
    }

    pub fn menubar(&mut self) {
        self.root
            .menubar()
            .add_leaf("Now Playing", |s| {
                s.call_on_name(
                    "screens",
                    |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                        screens.set_active_screen(0);
                    },
                );
            })
            .add_delimiter()
            .add_leaf("My Playlists", |s| {
                s.call_on_name(
                    "screens",
                    |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                        screens.set_active_screen(1);
                    },
                );
            })
            .add_delimiter()
            .add_leaf("Search", |s| {
                s.call_on_name(
                    "screens",
                    |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                        screens.set_active_screen(2);
                    },
                );
            });
    }

    pub async fn run(&mut self) {
        let player = self.player();
        let search = self.search();
        let my_playlists = self.my_playlists().await;

        let mut screens: ScreensView<ResizedView<LinearLayout>> = ScreensView::new();
        screens.add_active_screen(player.full_width());
        screens.add_screen(my_playlists.full_width());
        screens.add_screen(search.full_width());

        let full_layout = LinearLayout::vertical()
            .child(self.nav())
            .child(screens.with_name("screens"));

        self.root.add_fullscreen_layer(full_layout);

        self.global_events();
        self.root.run();
    }

    pub async fn sink(&self) -> &CbSink {
        self.root.cb_sink()
    }
}

fn submit_playlist(s: &mut Cursive, item: i64, client: Client, controls: Controls) {
    if let Ok(playlist) = block_on(async { client.playlist(item).await }) {
        if let (Some(tracks), Some(mut playlist_items)) = (
            playlist.tracks,
            s.find_name::<SelectView<(i32, Option<String>)>>("playlist_items"),
        ) {
            playlist_items.clear();

            for (i, t) in tracks.items.iter().enumerate() {
                if !t.streamable {
                    continue;
                }

                let mut row = StyledString::plain(format!("{:02} ", i));
                row.append(t.list_item());

                let value = if let Some(album) = &t.album {
                    (t.id, Some(album.id.clone()))
                } else {
                    (t.id, None)
                };

                playlist_items.add_item(row, value);
            }

            playlist_items.set_on_submit(move |s, item| {
                let c = controls.to_owned();
                submit_track(s, item.clone(), c);
            });
        }
    }
}

fn submit_artist(s: &mut Cursive, item: i32, client: Client, controls: Controls) {
    if let Ok(artist) = block_on(async { client.artist(item, Some(100)).await }) {
        if let Some(albums) = &artist.albums {
            let mut album_list: SelectView<String> = SelectView::new();

            for a in &albums.items {
                if !a.streamable {
                    continue;
                }

                album_list.add_item(a.list_item(), a.id.clone());

                let c = controls.to_owned();
                album_list.set_on_submit(move |s: &mut Cursive, item: &String| {
                    block_on(async { c.play_album(item.clone()).await });

                    s.screen_mut().pop_layer();
                });
            }

            album_list.sort_by_label();

            let mut events = OnEventView::new(
                album_list
                    .popup()
                    .scrollable()
                    .show_scrollbars(false)
                    .scroll_y(true),
            );

            events.set_on_pre_event(Event::Key(Key::Esc), |s| {
                s.screen_mut().pop_layer();
            });

            s.screen_mut().add_layer(events);
        }
    };
}

fn submit_track(s: &mut Cursive, item: (i32, Option<String>), controls: Controls) {
    let c = controls.to_owned();

    if item.1.is_none() {
        block_on(async { c.play_track(item.0).await });
        return;
    }

    let track = move |s: &mut Cursive| {
        s.screen_mut().pop_layer();

        let c = c.to_owned();
        block_on(async { c.play_track(item.0).await });
    };

    let album = move |s: &mut Cursive| {
        s.screen_mut().pop_layer();

        let c = controls.to_owned();
        if let Some(album_id) = &item.1 {
            block_on(async { c.play_album(album_id.clone()).await });
        }
    };

    let mut album_or_track = Dialog::text("Track or album?")
        .button("Track", track)
        .button("Album", album)
        .dismiss_button("Cancel")
        .wrap_with(OnEventView::new);

    album_or_track.set_on_pre_event(Event::Key(Key::Esc), |s| {
        s.screen_mut().pop_layer();
    });

    s.screen_mut().add_layer(album_or_track);
}

type CursiveSender = Sender<Box<dyn FnOnce(&mut Cursive) + Send>>;

pub async fn receive_notifications(cb: CursiveSender, mut receiver: BroadcastReceiver) {
    loop {
        select! {
            Ok(notification) = receiver.recv() => {
                match notification {
                    Notification::Status { status } => {
                        cb.send(Box::new(|s| {
                            if let Some(mut view) = s.find_name::<TextView>("player_status") {
                                match status.into() {
                                    GstState::Playing => {
                                        view.set_content(format!(" {}", '\u{25B6}'));
                                    }
                                    GstState::Paused => {
                                        view.set_content(format!(" {}", '\u{23F8}'));
                                    }
                                    GstState::Ready => {
                                        view.set_content("...");
                                    }
                                    GstState::Null => {
                                        view.set_content("Null");
                                    }
                                    _ => {}
                                }
                            }
                        })).expect("failed to send update");
                    }
                    Notification::Position { position } => {
                        cb.send(Box::new(move |s| {
                            if let Some(mut progress) = s.find_name::<ProgressBar>("progress") {
                                progress.set_value(position.inner_clocktime().seconds() as usize);
                            }
                        })).expect("failed to send update");
                    }
                    Notification::Duration {duration} => {
                        cb.send(Box::new(move |s| {
                            if let Some(mut progress) = s.find_name::<ProgressBar>("progress") {
                                progress.set_max(duration.inner_clocktime().seconds() as usize);
                            }
                        })).expect("failed to send update");
                    }
                    Notification::CurrentTrack {track} => {
                        cb.send(Box::new(move |s| {
                            if let (Some(mut track_num), Some(mut track_title), Some(mut progress)) = (s.find_name::<TextView>("current_track_number"), s.find_name::<TextView>("current_track_title"), s.find_name::<ProgressBar>("progress")) {
                                if track.album.is_some() {
                                    track_num.set_content(format!("{:02}", track.track.track_number));
                                } else {
                                    track_num.set_content(format!("{:02}", track.index));
                                }
                                track_title.set_content(track.track.title.trim());
                                progress.set_max(track.track.duration as usize);
                            }

                            if let (Some(track_url), Some(mut bit_depth), Some(mut sample_rate)) = (track.track_url, s.find_name::<TextView>("bit_depth"), s.find_name::<TextView>("sample_rate")) {
                                bit_depth.set_content(format!("{} bits", track_url.bit_depth));
                                sample_rate.set_content(format!("{} kHz", track_url.sampling_rate));
                            }
                        })).expect("failed to send update");
                    }
                    Notification::CurrentTrackList { list } => {
                        match list.list_type() {
                            TrackListType::Album => {
                                cb.send(Box::new(move |s| {
                                    if let Some(mut list_view) = s.find_name::<ScrollView<SelectView<usize>>>("current_track_list") {
                                        list_view.get_inner_mut().clear();

                                        list.unplayed_tracks().iter().for_each(|i| {
                                            list_view.get_inner_mut().add_item(i.track.track_list_item(false, None), i.index);
                                        });

                                        list.played_tracks().iter().for_each(|i| {
                                            list_view.get_inner_mut().add_item(i.track.track_list_item(true, None), i.index);
                                        });
                                    }
                                    if let (Some(album), Some(mut entity_title), Some(mut total_tracks)) = (list.get_album(), s.find_name::<TextView>("entity_title"), s.find_name::<TextView>("total_tracks")) {
                                        let year = chrono::NaiveDate::from_str(&album.release_date_original)
                                            .expect("failed to parse date")
                                            .format("%Y");

                                        let mut title = StyledString::plain(album.title.clone());
                                        title.append_plain(" ");
                                        title.append_styled(format!("({year})"), Effect::Dim);

                                        if let Some(mut artist_name) = s.find_name::<TextView>("artist_name") {
                                            artist_name.set_content(album.artist.name.clone());
                                        }

                                        entity_title.set_content(title);
                                        total_tracks.set_content(format!("{:02}", album.tracks_count));
                                    }
                                })).expect("failed to send update");
                            }
                            TrackListType::Playlist => {
                                cb.send(Box::new(move |s| {
                                    if let Some(mut list_view) = s.find_name::<ScrollView<SelectView<usize>>>("current_track_list") {
                                        list_view.get_inner_mut().clear();

                                        list.unplayed_tracks().iter().for_each(|i| {
                                            list_view.get_inner_mut().add_item(i.track.track_list_item(false, Some(i.index)), i.index);
                                        });

                                        list.played_tracks().iter().for_each(|i| {
                                            list_view.get_inner_mut().add_item(i.track.track_list_item(true, Some(i.index)), i.index);
                                        });
                                    }
                                    if let (Some(playlist), Some(mut entity_title), Some(mut total_tracks)) = (list.get_playlist(), s.find_name::<TextView>("entity_title"), s.find_name::<TextView>("total_tracks")) {
                                        entity_title.set_content(playlist.name.clone());
                                        total_tracks.set_content(playlist.tracks_count.to_string());
                                    }
                                })).expect("failed to send update");
                            }
                            TrackListType::Track => {
                                cb.send(Box::new(move |s| {
                                    if let Some(mut list_view) = s.find_name::<ScrollView<SelectView<usize>>>("current_track_list") {
                                        list_view.get_inner_mut().clear();
                                    }

                                    if let (Some(album), Some(mut entity_title)) = (list.get_album(), s.find_name::<TextView>("entity_title")) {
                                        entity_title.set_content(album.title.trim());
                                    }
                                    if let Some(mut total_tracks) = s.find_name::<TextView>("total_tracks") {
                                        total_tracks.set_content("00");
                                    }
                                })).expect("failed to send update");
                            }
                            _ => {}
                        }
                    }
                    Notification::Buffering { is_buffering: _ } => {
                        // cb.send(Box::new(move |s| {
                        //     s.call_on_name("player_panel", |panel: &mut Panel<LinearLayout>| {
                        //         debug!("player_panel **************************");
                        //         if is_buffering {
                        //             panel.set_title("player b");
                        //         }else {
                        //             panel.set_title("player x");
                        //         }
                        //     });
                        // })).expect("failed to send update");
                    },
                    Notification::Error { error: _ } => {

                    }
                }
            }
            else => {}
        }
    }
}

pub trait CursiveFormat {
    fn list_item(&self) -> StyledString;
    fn track_list_item(&self, _inactive: bool, _index: Option<usize>) -> StyledString {
        StyledString::new()
    }
}
