use crate::{
    player::{controls::Controls, notification::BroadcastReceiver, notification::Notification},
    state::TrackListType,
};
use cursive::{
    align::HAlign,
    direction::Orientation,
    event::Event,
    reexports::crossbeam_channel::Sender,
    theme::{Effect, Style},
    utils::{markup::StyledString, Counter},
    view::{Nameable, Resizable, Scrollable, SizeConstraint},
    views::{
        Dialog, EditView, LinearLayout, NamedView, PaddedView, Panel, ProgressBar, RadioGroup,
        ResizedView, ScreensView, ScrollView, SelectView, TextView,
    },
    CbSink, Cursive, CursiveRunnable, With,
};
use futures::executor::block_on;
use gstreamer::{ClockTime, State as GstState};
use hifirs_qobuz_api::client::{api::Client, search_results::SearchAllResults};
use std::str::FromStr;
use tokio::select;

pub struct CursiveUI<'c> {
    root: CursiveRunnable,
    controls: &'c Controls,
    client: Client,
}

impl<'c> CursiveUI<'c> {
    pub fn new(controls: &'c Controls, client: Client) -> Self {
        let mut siv = cursive::default();

        siv.set_autohide_menu(false);

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
                        .scroll_x(true),
                )
                .child(TextView::new("").with_name("artist_name"))
                .child(
                    TextView::new("")
                        .with_name("entity_title")
                        .scrollable()
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
                .button("No", |s| {
                    s.pop_layer();
                });

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

        let mut user_playlists = SelectView::new();
        let playlist_items: SelectView<(i32, Option<String>)> = SelectView::new();

        if let Ok(my_playlists) = self.client.user_playlists().await {
            my_playlists.playlists.items.iter().for_each(|p| {
                user_playlists.add_item(p.name.clone(), p.id as i32);
            });
        }

        let c = self.controls.to_owned();
        let client = self.client.clone();

        user_playlists.set_on_submit(move |s, item| {
            let client = client.clone();
            let id = *item as i64;

            let c1 = c.to_owned();
            let open = move |s: &mut Cursive| {
                if let Ok(playlist) = block_on(async { client.playlist(id).await }) {
                    if let (Some(tracks), Some(mut playlist_items)) = (
                        playlist.tracks,
                        s.find_name::<ScrollView<SelectView<(i32, Option<String>)>>>(
                            "playlist_items",
                        ),
                    ) {
                        playlist_items.get_inner_mut().clear();

                        for (i, t) in tracks.items.iter().enumerate() {
                            if !t.streamable {
                                continue;
                            }

                            let mut row = StyledString::plain(format!("{:02} ", i));
                            row.append_styled(t.title.trim(), Effect::Bold);

                            if let Some(performer) = &t.performer {
                                row.append_plain(" by ");
                                row.append_plain(performer.name.clone());
                            };

                            row.append_plain(" ");

                            let duration = ClockTime::from_seconds(t.duration as u64)
                                .to_string()
                                .as_str()[2..7]
                                .to_string();
                            row.append_styled(duration, Effect::Dim);
                            row.append_plain(" ");

                            if t.parental_warning {
                                row.append_styled("e", Effect::Dim);
                            }

                            if t.hires_streamable {
                                row.append_styled("*", Effect::Dim);
                            }

                            let value = if let Some(album) = &t.album {
                                (t.id, Some(album.id.clone()))
                            } else {
                                (t.id, None)
                            };

                            playlist_items.get_inner_mut().add_item(row, value);
                        }

                        let c = c1.to_owned();
                        playlist_items
                            .get_inner_mut()
                            .set_on_submit(move |s, item| {
                                let c = c.to_owned();
                                let c1 = c.to_owned();

                                let item = item.to_owned();
                                let track = move |s: &mut Cursive| {
                                    let c = c.to_owned();

                                    block_on(async move { c.play_track(item.0).await });

                                    s.screen_mut().pop_layer();
                                };

                                let album = move |s: &mut Cursive| {
                                    if let Some(album_id) = &item.1 {
                                        let c = c1.to_owned();

                                        block_on(
                                            async move { c.play_album(album_id.clone()).await },
                                        );

                                        s.screen_mut().pop_layer();
                                    }
                                };

                                let album_or_track = Dialog::text("Track or album?")
                                    .button("Track", track)
                                    .button("Album", album)
                                    .dismiss_button("Cancel");

                                s.screen_mut().add_layer(album_or_track);
                            });
                    }
                }

                s.screen_mut().pop_layer();
            };

            let c2 = c.to_owned();
            let play = move |s: &mut Cursive| {
                let c = c2.to_owned();
                block_on(async move { c.play_playlist(id).await });
                s.pop_layer();
            };

            let dialog = Dialog::text("Open or play?")
                .button("Open", open)
                .button("Play", play)
                .dismiss_button("Cancel");

            s.screen_mut().add_layer(dialog);
        });

        list_layout.add_child(
            Panel::new(
                user_playlists
                    .with_name("user_playlists")
                    .scrollable()
                    .scroll_y(true),
            )
            .title("my playlists")
            .max_height(10),
        );
        list_layout.add_child(
            Panel::new(
                playlist_items
                    .scrollable()
                    .scroll_y(true)
                    .with_name("playlist_items"),
            )
            .full_height(),
        );

        list_layout
    }

    fn search(&self) -> LinearLayout {
        let mut layout = LinearLayout::new(Orientation::Vertical);
        let c = self.controls.to_owned();
        let client = self.client.to_owned();

        let on_change = move |s: &mut Cursive, item: &String| {
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
                                let mut title = StyledString::styled(a.title.clone(), Effect::Bold);
                                title.append_plain(" by ");
                                title.append_plain(a.artist.name.clone());
                                title.append_plain(" ");

                                let year = chrono::NaiveDate::from_str(&a.release_date_original)
                                    .expect("failed to parse date")
                                    .format("%Y");

                                title.append_styled(year.to_string(), Effect::Dim);
                                title.append_plain(" ");

                                if a.parental_warning {
                                    title.append_styled("e", Effect::Dim);
                                }

                                if a.hires_streamable {
                                    title.append_styled("*", Effect::Dim);
                                }

                                results.add_item(title, a.id.clone());
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
                                if let Ok(artist_albums) = block_on(async {
                                    client
                                        .artist(
                                            item.parse::<i32>().expect("failed to parse string"),
                                            Some(100),
                                        )
                                        .await
                                }) {
                                    if let (Some(mut search_results), Some(albums)) = (
                                        s.find_name::<SelectView<String>>("search_results"),
                                        artist_albums.albums,
                                    ) {
                                        search_results.clear();
                                        for a in &albums.items {
                                            if !a.streamable {
                                                continue;
                                            }
                                            let year = chrono::NaiveDate::from_str(
                                                &a.release_date_original,
                                            )
                                            .expect("failed to parse date")
                                            .format("%Y");

                                            let mut row = StyledString::plain(year.to_string());
                                            row.append_plain(" ");
                                            row.append_styled(a.title.clone(), Effect::Bold);
                                            row.append_plain(" ");

                                            if a.parental_warning {
                                                row.append_styled("e", Effect::Dim);
                                            }

                                            if a.hires_streamable {
                                                row.append_styled("*", Effect::Dim);
                                            }

                                            search_results.add_item(row, a.id.clone());
                                        }

                                        let artist_name =
                                            albums.items.first().unwrap().artist.name.clone();
                                        s.call_on_name(
                                            "results_panel",
                                            |panel: &mut Panel<
                                                ScrollView<NamedView<SelectView<String>>>,
                                            >| {
                                                let mut title = StyledString::plain("albums by ");
                                                title.append_styled(artist_name, Effect::Bold);

                                                panel.set_title(title)
                                            },
                                        );

                                        let c = c.to_owned();
                                        search_results.set_on_submit(
                                            move |_s: &mut Cursive, item: &String| {
                                                block_on(async { c.play_album(item.clone()).await })
                                            },
                                        );
                                    }
                                };
                            });
                        }
                        "Tracks" => {
                            for t in &data.tracks.items {
                                if !t.streamable {
                                    continue;
                                }
                                if let Some(performer) = &t.performer {
                                    let mut title =
                                        StyledString::styled(t.title.clone(), Effect::Bold);
                                    title.append_plain(" by ");
                                    title.append_plain(performer.name.clone());

                                    let duration = ClockTime::from_seconds(t.duration as u64)
                                        .to_string()
                                        .as_str()[2..7]
                                        .to_string();
                                    title.append_plain(" ");
                                    title.append_styled(duration, Effect::Dim);
                                    title.append_plain(" ");

                                    if t.parental_warning {
                                        title.append_styled("e", Effect::Dim);
                                    }

                                    if t.hires_streamable {
                                        title.append_styled("*", Effect::Dim);
                                    }

                                    results.add_item(title, t.id.to_string())
                                } else {
                                    results.add_item(t.title.clone(), t.id.to_string())
                                }
                            }

                            let c = c.to_owned();
                            results.set_on_submit(move |_s: &mut Cursive, item: &String| {
                                block_on(async {
                                    c.play_track(
                                        item.parse::<i32>().expect("failed to parse string"),
                                    )
                                    .await
                                })
                            });
                        }
                        "Playlists" => {
                            data.playlists
                                .items
                                .iter()
                                .for_each(|p| results.add_item(p.name.clone(), p.id.to_string()));
                        }
                        _ => {}
                    }
                }
            }
        };

        let mut search_type = RadioGroup::new().on_change(on_change);

        let radios = LinearLayout::horizontal()
            .child(search_type.button_str("Albums"))
            .child(search_type.button_str("Artists"))
            .child(search_type.button_str("Tracks"))
            .child(search_type.button_str("Playlists"))
            .wrap_with(Panel::new);

        let c = self.client.clone();
        let search_form = EditView::new()
            .on_submit_mut(move |s, item| {
                if let Ok(results) = block_on(async { c.search_all(item.to_string()).await }) {
                    debug!("saving search results to user data");
                    s.set_user_data(results);
                }
            })
            .wrap_with(Panel::new);

        let search_results: NamedView<SelectView<String>> =
            SelectView::new().with_name("search_results");

        layout.add_child(search_form.title("search"));
        layout.add_child(radios);
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
        self.root.add_fullscreen_layer(screens.with_name("screens"));

        self.global_events();
        self.menubar();
        self.root.run();
    }

    pub async fn sink(&self) -> &CbSink {
        self.root.cb_sink()
    }
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
                                track_title.set_content(track.track.title);
                                progress.set_max(track.track.duration as usize);
                            }

                            if let (Some(performer),Some(mut artist_name)) = (track.track.performer, s.find_name::<TextView>("artist_name")) {
                                artist_name.set_content(performer.name);
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
                                            let mut row = StyledString::plain(format!("{:02} ", i.track.track_number));
                                            row.append_styled(i.track.title.trim(), Effect::Bold);
                                            row.append_plain(" ");

                                            let duration = ClockTime::from_seconds(i.track.duration as u64)
                                                .to_string()
                                                .as_str()[2..7]
                                                .to_string();
                                            row.append_styled(duration, Effect::Dim);


                                            list_view.get_inner_mut().add_item(row, i.index);
                                        });

                                        list.played_tracks().iter().for_each(|i| {
                                            let mut row = StyledString::styled(format!("{:02} ", i.track.track_number), Effect::Dim);
                                            row.append_styled(i.track.title.trim(), Style::from(Effect::Dim).combine(Effect::Italic));
                                            row.append_plain(" ");

                                            let duration = ClockTime::from_seconds(i.track.duration as u64)
                                                .to_string()
                                                .as_str()[2..7]
                                                .to_string();
                                            row.append_styled(duration, Effect::Dim);

                                            list_view.get_inner_mut().add_item(row, i.index);
                                        });
                                    }
                                    if let (Some(album), Some(mut entity_title), Some(mut total_tracks)) = (list.get_album(), s.find_name::<TextView>("entity_title"), s.find_name::<TextView>("total_tracks")) {
                                        entity_title.set_content(album.title.clone());
                                        total_tracks.set_content(format!("{:02}", album.tracks_count));
                                    }
                                })).expect("failed to send update");
                            }
                            TrackListType::Playlist => {
                                cb.send(Box::new(move |s| {
                                    if let (Some(playlist), Some(mut entity_title), Some(mut total_tracks)) = (list.get_playlist(), s.find_name::<TextView>("entity_title"), s.find_name::<TextView>("total_tracks")) {
                                        entity_title.set_content(playlist.name.clone());
                                        total_tracks.set_content(playlist.tracks_count.to_string());
                                    }
                                })).expect("failed to send update");
                            }
                            TrackListType::Track => {
                                cb.send(Box::new(move |s| {
                                    if let (Some(album), Some(mut entity_title)) = (list.get_album(), s.find_name::<TextView>("entity_title")) {
                                        entity_title.set_content(album.title.clone());
                                    }
                                    if let Some(mut total_tracks) = s.find_name::<TextView>("total_tracks") {
                                        total_tracks.set_content("00");
                                    }
                                })).expect("failed to send update");
                            }
                            _ => {}
                        }
                    }
                    Notification::Buffering { is_buffering } => {
                        cb.send(Box::new(move |s| {
                            s.call_on_name("player_panel", |panel: &mut Panel<LinearLayout>| {
                                debug!("player_panel **************************");
                                if is_buffering {
                                    panel.set_title("player b");
                                }else {
                                    panel.set_title("player x");
                                }
                            });
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
