use crate::{
    player::{controls::Controls, notification::BroadcastReceiver, notification::Notification},
    state::TrackListType,
};
use cursive::{
    align::HAlign,
    direction::Orientation,
    event::Event,
    immut1,
    reexports::crossbeam_channel::Sender,
    theme::Effect,
    utils::Counter,
    view::{Nameable, Resizable, Scrollable, SizeConstraint},
    views::{
        Dialog, DummyView, LinearLayout, NamedView, Panel, ProgressBar, ResizedView, ScrollView,
        SelectView, TextView,
    },
    CbSink, Cursive, CursiveRunnable,
};
use futures::executor::block_on;
use gstreamer::{ClockTime, State as GstState};
use hifirs_qobuz_api::client::api::Client;
use tokio::select;

pub struct CursiveUI<'c> {
    root: CursiveRunnable,
    controls: &'c Controls,
    client: Client,
}

type ItemList = ResizedView<Panel<ScrollView<NamedView<SelectView<i32>>>>>;
type PlaylistView = NamedView<Panel<ScrollView<SelectView<(i32, Option<String>)>>>>;

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

    pub fn player() -> ResizedView<NamedView<Panel<ResizedView<LinearLayout>>>> {
        let mut container = LinearLayout::new(Orientation::Vertical);
        let mut track_info = LinearLayout::new(Orientation::Horizontal);

        let meta = LinearLayout::new(Orientation::Vertical)
            .child(
                TextView::new("")
                    .style(Effect::Bold)
                    .with_name("current_track_title"),
            )
            .child(TextView::new("").with_name("artist_name"))
            .child(TextView::new("").with_name("entity_title"));

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
                TextView::new("X")
                    .h_align(HAlign::Center)
                    .with_name("player_status"),
            )
            .child(
                TextView::new("44.1")
                    .h_align(HAlign::Center)
                    .with_name("sample_rate"),
            )
            .child(
                TextView::new("24")
                    .h_align(HAlign::Center)
                    .with_name("bit_depth"),
            )
            .fixed_width(4);

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
            .with_name("progress")
            .full_width();

        track_info.add_child(track_num);
        track_info.add_child(meta.full_width());
        track_info.add_child(player_status);

        container.add_child(track_info.full_width());
        container.add_child(progress);

        Panel::new(container.full_width())
            .title("player")
            .with_name("player_panel")
            .resized(SizeConstraint::Full, SizeConstraint::Free)
    }

    pub fn global_events(&mut self) {
        self.root.clear_global_callbacks(Event::CtrlChar('c'));

        let c = self.controls.clone();
        self.root.set_on_pre_event(Event::CtrlChar('c'), move |s| {
            let c = c.clone();

            let dialog = Dialog::text("Do you want to quit?")
                .button(
                    "Yes",
                    immut1!(move |s: &mut Cursive| {
                        c.quit_blocking();
                        s.quit();
                    }),
                )
                .button("No", |s| {
                    s.pop_layer();
                });

            s.add_layer(dialog);
        });

        let c = self.controls.clone();
        self.root.add_global_callback('p', move |_| {
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
    }

    pub async fn my_playlists(&self) -> ResizedView<NamedView<LinearLayout>> {
        let mut list_layout = LinearLayout::new(Orientation::Vertical);

        let player = CursiveUI::player();
        list_layout.add_child(player);
        list_layout.add_child(DummyView.resized(SizeConstraint::Full, SizeConstraint::Fixed(1)));

        let mut user_playlists: ItemList = Panel::new(
            SelectView::new()
                .with_name("user_playlists")
                .scrollable()
                .scroll_y(true),
        )
        .title("my playlists")
        .max_height(10);

        if let Ok(my_playlists) = self.client.user_playlists().await {
            let mut track_list = user_playlists
                .get_inner_mut()
                .get_inner_mut()
                .get_inner_mut()
                .get_mut();

            my_playlists.playlists.items.iter().for_each(|p| {
                track_list.add_item(p.name.clone(), p.id as i32);
            });

            let c = self.controls.to_owned();
            let client = self.client.clone();

            track_list.set_on_submit(move |s, item| {
                let client = client.clone();
                let id = *item as i64;

                let c = c.clone();
                let c2 = c.clone();
                let dialog = Dialog::text("Open or play?")
                    .button("Open", move |s| {
                        if let Ok(playlist) = block_on(async { client.playlist(id).await }) {
                            if let Some(tracks) = playlist.tracks {
                                let mut view: PlaylistView =
                                    Panel::new(SelectView::new().scrollable().scroll_y(true))
                                        .title(playlist.name)
                                        .with_name("playlist_items");

                                tracks.items.iter().enumerate().for_each(|(i, t)| {
                                    let row = format!("{:02} {}", i, t.title.clone());
                                    let value = if let Some(album) = &t.album {
                                        (t.id, Some(album.id.clone()))
                                    } else {
                                        (t.id, None)
                                    };
                                    view.get_mut()
                                        .get_inner_mut()
                                        .get_inner_mut()
                                        .add_item(row, value);
                                });

                                let c = c.to_owned();
                                view.get_mut()
                                    .get_inner_mut()
                                    .get_inner_mut()
                                    .set_on_submit(move |_s, item| {
                                        let c = c.to_owned();
                                        block_on(async move { c.play_track(item.0).await });
                                    });

                                if let Some(mut layout) =
                                    s.find_name::<LinearLayout>("user_playlist_layout")
                                {
                                    layout.add_child(view);
                                }
                            }
                        }

                        s.screen_mut().pop_layer();
                    })
                    .button("Play", move |s| {
                        let c = c2.to_owned();
                        block_on(async move { c.play_playlist(id).await });
                        s.pop_layer();
                    })
                    .dismiss_button("Cancel");

                s.screen_mut().add_layer(dialog);
            });
        }

        list_layout.add_child(user_playlists);

        list_layout.with_name("user_playlist_layout").full_height()
    }

    pub fn menubar(&mut self) {
        let menu = self.root.menubar();

        menu.add_subtree(
            "My Playlists",
            cursive::menu::Tree::new().leaf("Open", |_s| {}),
        );
    }

    pub async fn run(&mut self) {
        self.global_events();
        self.menubar();

        // let theme = Theme {
        //     shadow: false,
        //     ..Default::default()
        // };

        let my_playlists = self.my_playlists().await;

        self.root.screen_mut().add_fullscreen_layer(my_playlists);

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
                                        view.set_content("▶");
                                    }
                                    GstState::Paused => {
                                        view.set_content("∥");
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
                                track_num.set_content(format!("{:02}", track.index));
                                track_title.set_content(track.track.title);
                                progress.set_max(track.track.duration as usize);
                            }

                            if let (Some(performer),Some(mut artist_name)) = (track.track.performer, s.find_name::<TextView>("artist_name")) {
                                artist_name.set_content(performer.name);
                            }

                            if let (Some(track_url), Some(mut bit_depth), Some(mut sample_rate)) = (track.track_url, s.find_name::<TextView>("bit_depth"), s.find_name::<TextView>("sample_rate")) {
                                bit_depth.set_content(track_url.bit_depth.to_string());
                                sample_rate.set_content(track_url.sampling_rate.to_string());
                            }
                        })).expect("failed to send update");
                    }
                    Notification::CurrentTrackList { list } => {
                        match list.list_type() {
                            TrackListType::Album => {
                                cb.send(Box::new(move |s| {
                                    if let Some(mut list_view) = s.find_name::<SelectView<i32>>("track_list") {
                                        list.vec().into_iter().for_each(|i| {
                                            list_view.add_item(i.track.title.clone(), i.track.id);
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
                            if let Some(mut view) = s.find_name::<Panel<ResizedView<LinearLayout>>>("player_planel") {
                                if is_buffering {
                                    view.set_title("player b");
                                }else {
                                    view.set_title("player x");
                                }
                            }
                        })).expect("failed to send update");
                    },
                    Notification::Error { error: _ } => {

                    }
                }
            }
        }
    }
}
