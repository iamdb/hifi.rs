use std::{rc::Rc, str::FromStr};

use crate::{
    player::{controls::Controls, notification::BroadcastReceiver, notification::Notification},
    state::TrackListType,
};
use cursive::{
    align::HAlign,
    direction::Orientation,
    event::{Event, Key},
    reexports::crossbeam_channel::Sender,
    theme::{BorderStyle, Effect, Palette, Style},
    utils::{markup::StyledString, Counter},
    view::{Nameable, Resizable, Scrollable, SizeConstraint},
    views::{
        Button, Dialog, EditView, HideableView, LinearLayout, MenuPopup, NamedView, OnEventView,
        PaddedView, Panel, ProgressBar, ResizedView, ScreensView, ScrollView, SelectView, TextView,
    },
    CbSink, Cursive, CursiveRunnable, With,
};
use futures::executor::block_on;
use gstreamer::{ClockTime, State as GstState};
use hifirs_qobuz_api::client::{api::Client, search_results::SearchAllResults};
use tokio::select;

static UNSTREAMABLE: &str = "UNSTREAMABLE";

pub struct CursiveUI {
    root: CursiveRunnable,
    controls: Controls,
    client: Client,
}

impl CursiveUI {
    pub fn new(controls: Controls, client: Client) -> Self {
        let mut siv = cursive::default();

        siv.set_theme(cursive::theme::Theme {
            shadow: false,
            borders: BorderStyle::Simple,
            palette: Palette::terminal_default().with(|palette| {
                use cursive::theme::BaseColor::*;

                {
                    // First, override some colors from the base palette.
                    use cursive::theme::Color::TerminalDefault;
                    use cursive::theme::PaletteColor::*;

                    palette[Background] = TerminalDefault;
                    palette[View] = TerminalDefault;
                    palette[Primary] = White.dark();
                    palette[Highlight] = Cyan.dark();
                    palette[HighlightInactive] = Black.dark();
                    palette[HighlightText] = Black.dark();
                }

                {
                    // Then override some styles.
                    use cursive::theme::Color::TerminalDefault;
                    use cursive::theme::Effect::*;
                    use cursive::theme::PaletteStyle::*;

                    palette[Highlight] = Style::from(Cyan.dark())
                        .combine(Underline)
                        .combine(Reverse)
                        .combine(Bold);
                    palette[HighlightInactive] = Style::from(TerminalDefault).combine(Reverse);
                    palette[TitlePrimary] = Style::from(Cyan.dark()).combine(Bold);
                }
            }),
        });

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
                        .style(Style::highlight().combine(Effect::Bold))
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
        )
        .resized(SizeConstraint::Full, SizeConstraint::Free);

        let track_num = LinearLayout::new(Orientation::Vertical)
            .child(
                TextView::new("000")
                    .h_align(HAlign::Left)
                    .with_name("current_track_number"),
            )
            .child(TextView::new("of").h_align(HAlign::Center))
            .child(
                TextView::new("000")
                    .h_align(HAlign::Left)
                    .with_name("total_tracks"),
            )
            .fixed_width(3);

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
        track_info.add_child(meta);
        track_info.add_child(player_status);

        container.add_child(track_info);
        container.add_child(progress);

        let mut track_list: SelectView<usize> = SelectView::new();

        let c = self.controls.clone();
        track_list.set_on_submit(move |_s, item| {
            block_on(async { c.skip_to(*item).await });
        });

        let mut layout = LinearLayout::new(Orientation::Vertical).child(
            Panel::new(container)
                .title("player")
                .with_name("player_panel"),
        );

        layout.add_child(Panel::new(
            HideableView::new(
                track_list
                    .scrollable()
                    .scroll_y(true)
                    .scroll_x(true)
                    .with_name("current_track_list"),
            )
            .visible(true),
        ));

        layout
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
            s.set_screen(0);
        });
        self.root.add_global_callback('2', move |s| {
            s.set_screen(1);
        });

        self.root.add_global_callback('3', move |s| {
            s.set_screen(2);
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

    pub async fn my_playlists(&self) -> NamedView<LinearLayout> {
        let mut list_layout = LinearLayout::new(Orientation::Vertical);

        let mut user_playlists = SelectView::new().popup();
        user_playlists.add_item("Select Playlist", 0);

        if let Ok(my_playlists) = self.client.user_playlists().await {
            my_playlists.playlists.items.iter().for_each(|p| {
                user_playlists.add_item(p.name.clone(), p.id);
            });
        }

        let c = self.controls.clone();
        let client = self.client.clone();
        user_playlists.set_on_submit(move |s: &mut Cursive, item: &i64| {
            if item == &0 {
                s.call_on_name("play_button", |button: &mut Button| {
                    button.disable();
                });

                return;
            }

            let c = c.clone();
            let client = client.clone();

            let layout = submit_playlist(s, *item, client, c).wrap_with(Panel::new);

            s.call_on_name("user_playlist_layout", |l: &mut LinearLayout| {
                l.remove_child(1);
                l.add_child(layout);
            });

            s.call_on_name("play_button", |button: &mut Button| {
                button.enable();
            });
        });

        list_layout.add_child(
            Panel::new(
                user_playlists
                    .with_name("user_playlists")
                    .scrollable()
                    .scroll_y(true)
                    .resized(SizeConstraint::Full, SizeConstraint::Free),
            )
            .title("my playlists"),
        );

        list_layout.with_name("user_playlist_layout")
    }

    fn search(&mut self) -> LinearLayout {
        let mut layout = LinearLayout::new(Orientation::Vertical);

        let c = self.controls.to_owned();
        let client = self.client.to_owned();

        let on_submit = move |s: &mut Cursive, item: &String| {
            let item = item.clone();

            if let Some(mut search_results) = s.find_name::<SelectView>("search_results") {
                search_results.clear();

                if let Some(data) = s.user_data::<SearchAllResults>() {
                    match item.as_str() {
                        "Albums" => {
                            for a in &data.albums.items {
                                let id = if a.streamable {
                                    a.id.clone()
                                } else {
                                    UNSTREAMABLE.to_string()
                                };

                                search_results.add_item(a.list_item(), id);
                            }

                            let c = c.to_owned();
                            search_results.set_on_submit(move |_s: &mut Cursive, item: &String| {
                                if item != UNSTREAMABLE {
                                    block_on(async { c.play_album(item.clone()).await })
                                }
                            });
                        }
                        "Artists" => {
                            for a in &data.artists.items {
                                search_results.add_item(a.name.clone(), a.id.to_string());
                            }

                            let client = client.to_owned();
                            let c = c.to_owned();
                            search_results.set_on_submit(move |s: &mut Cursive, item: &String| {
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
                                let id = if t.streamable {
                                    t.id.to_string()
                                } else {
                                    UNSTREAMABLE.to_string()
                                };

                                search_results.add_item(t.list_item(), id)
                            }

                            let c = c.to_owned();
                            search_results.set_on_submit(move |s: &mut Cursive, item: &String| {
                                if item != UNSTREAMABLE {
                                    let c = c.to_owned();
                                    submit_track(
                                        s,
                                        (
                                            item.parse::<i32>().expect("failed to parse string"),
                                            None,
                                        ),
                                        c,
                                    );
                                }
                            });
                        }
                        "Playlists" => {
                            for p in &data.playlists.items {
                                search_results.add_item(p.name.clone(), p.id.to_string())
                            }

                            let c = c.to_owned();
                            let client = client.to_owned();
                            search_results.set_on_submit(move |s: &mut Cursive, item: &String| {
                                let c = c.to_owned();
                                let client = client.to_owned();

                                let layout = submit_playlist(
                                    s,
                                    item.parse::<i64>().expect("failed to parse string"),
                                    client,
                                    c,
                                );

                                let event_panel = OnEventView::new(layout).on_event(
                                    Event::Key(Key::Esc),
                                    move |s| {
                                        s.screen_mut().pop_layer();
                                    },
                                );

                                s.screen_mut().add_layer(Panel::new(event_panel));
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
                if let Ok(results) = block_on(async { c.search_all(item.to_string(), 100).await }) {
                    debug!("saving search results to user data");
                    s.set_user_data(results);

                    if let Some(view) = s.find_name::<SelectView>("search_type") {
                        if let Some(value) = view.selection() {
                            on_submit(s, &value.to_string());
                        }
                    }
                }
            })
            .wrap_with(Panel::new);

        let search_results: SelectView<String> = SelectView::new();

        layout.add_child(search_form.title("search"));
        layout.add_child(search_type);

        layout.add_child(
            Panel::new(
                search_results
                    .with_name("search_results")
                    .scrollable()
                    .scroll_y(true)
                    .scroll_x(true)
                    .resized(SizeConstraint::Free, SizeConstraint::Full),
            )
            .title("results"),
        );

        layout
    }

    fn results_list(name: &str) -> ResultsPanel {
        let panel: ResultsPanel = SelectView::new()
            .with_name(name)
            .scrollable()
            .scroll_y(true)
            .scroll_x(true);

        panel
    }

    pub fn menubar(&mut self) {
        self.root.set_autohide_menu(false);

        self.root
            .menubar()
            .add_leaf(
                StyledString::styled("Now Playing", Effect::Underline),
                |s| {
                    s.set_screen(0);
                },
            )
            .add_delimiter()
            .add_leaf(
                StyledString::styled("My Playlists", Effect::Underline),
                |s| {
                    s.set_screen(1);
                },
            )
            .add_delimiter()
            .add_leaf(StyledString::styled("Search", Effect::Underline), |s| {
                s.set_screen(2);
            });
    }

    pub async fn run(&mut self) {
        let player = self.player();
        let search = self.search();
        let my_playlists = self.my_playlists().await;

        self.root
            .screen_mut()
            .add_fullscreen_layer(PaddedView::lrtb(
                0,
                0,
                1,
                0,
                player.resized(SizeConstraint::Full, SizeConstraint::Free),
            ));

        self.root.add_active_screen();
        self.root
            .screen_mut()
            .add_fullscreen_layer(PaddedView::lrtb(
                0,
                0,
                1,
                0,
                my_playlists.resized(SizeConstraint::Full, SizeConstraint::Free),
            ));

        self.root.add_active_screen();
        self.root
            .screen_mut()
            .add_fullscreen_layer(PaddedView::lrtb(
                0,
                0,
                1,
                0,
                search.resized(SizeConstraint::Full, SizeConstraint::Free),
            ));

        self.root.set_screen(0);

        self.menubar();
        self.global_events();
        self.root.run();
    }

    pub async fn sink(&self) -> &CbSink {
        self.root.cb_sink()
    }
}

type ResultsPanel = ScrollView<NamedView<SelectView<(i32, Option<String>)>>>;

fn submit_playlist(
    _s: &mut Cursive,
    item: i64,
    client: Client,
    controls: Controls,
) -> LinearLayout {
    let mut layout = LinearLayout::vertical();

    if let Ok(playlist) = block_on(async { client.playlist(item).await }) {
        if let Some(tracks) = playlist.tracks {
            let mut list = CursiveUI::results_list("playlist_items");
            let mut playlist_items = list.get_inner_mut().get_mut();

            for (i, t) in tracks.items.iter().enumerate() {
                let mut row = StyledString::plain(format!("{:02} ", i));
                row.append(t.list_item());

                let track_id = if t.streamable { t.id } else { -1 };

                let value = if let Some(album) = &t.album {
                    let album_id = if album.streamable {
                        album.id.clone()
                    } else {
                        UNSTREAMABLE.to_string()
                    };

                    (track_id, Some(album_id))
                } else {
                    (track_id, None)
                };

                playlist_items.add_item(row, value);
            }

            let c = controls.clone();
            playlist_items.set_on_submit(move |s, item| {
                let c = c.clone();
                submit_track(s, item.clone(), c);
            });

            let c = controls;
            let meta = LinearLayout::horizontal()
                .child(Button::new("play", move |_s| {
                    let c = c.clone();

                    block_on(async { c.play_playlist(playlist.id).await });
                }))
                .child(
                    TextView::new(format!("total tracks: {}", playlist.tracks_count))
                        .h_align(HAlign::Right)
                        .full_width(),
                );

            layout.add_child(meta);
            layout.add_child(list);
        }
    }

    layout
}

fn submit_artist(s: &mut Cursive, item: i32, client: Client, controls: Controls) {
    if let Ok(artist) = block_on(async { client.artist(item, Some(100)).await }) {
        if let Some(mut albums) = artist.albums {
            albums
                .items
                .sort_by_key(|a| a.release_date_original.to_owned());

            let mut tree = cursive::menu::Tree::new();

            for a in albums.items {
                if !a.streamable {
                    continue;
                }

                let c = controls.to_owned();
                tree.add_leaf(a.list_item(), move |s: &mut Cursive| {
                    block_on(async { c.play_album(a.id.clone()).await });

                    s.call_on_name(
                        "screens",
                        |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                            screens.set_active_screen(0);
                        },
                    );
                });
            }

            let album_list: MenuPopup = MenuPopup::new(Rc::new(tree));

            let events = album_list
                .scrollable()
                .resized(SizeConstraint::Full, SizeConstraint::Free);

            s.screen_mut().add_layer(events);
        }
    };
}

fn submit_track(s: &mut Cursive, item: (i32, Option<String>), controls: Controls) {
    if item.0 == -1 {
        return;
    }

    let c = controls.to_owned();

    if item.1.is_none() {
        block_on(async { c.play_track(item.0).await });

        s.call_on_name(
            "screens",
            |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                screens.set_active_screen(0);
            },
        );
        return;
    }

    let track = move |s: &mut Cursive| {
        s.screen_mut().pop_layer();

        let c = c.to_owned();
        block_on(async { c.play_track(item.0).await });

        s.call_on_name(
            "screens",
            |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                screens.set_active_screen(0);
            },
        );
    };

    let album = move |s: &mut Cursive| {
        s.screen_mut().pop_layer();

        let c = controls.to_owned();
        if let Some(album_id) = &item.1 {
            block_on(async { c.play_album(album_id.clone()).await });

            s.call_on_name(
                "screens",
                |screens: &mut ScreensView<ResizedView<LinearLayout>>| {
                    screens.set_active_screen(0);
                },
            );
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
                                    track_num.set_content(format!("{:03}", track.track.track_number));
                                } else {
                                    track_num.set_content(format!("{:03}", track.index));
                                }
                                track_title.set_content(track.track.title.trim());
                                progress.set_max(track.track.duration as usize);
                            }

                            if let Some(performer) = track.track.performer {
                                s.call_on_name("artist_name", |view: &mut TextView| {
                                    view.set_content(performer.name);
                                });
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
                        cb.send(Box::new(move |_s| {

                        })).expect("failed to send update");
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
