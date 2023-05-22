use crate::player::{BroadcastReceiver, Controls, Notification};
use cursive::{
    align::HAlign,
    direction::Orientation,
    reexports::crossbeam_channel::Sender,
    theme::{Effect, Theme},
    utils::Counter,
    view::{Nameable, Resizable, SizeConstraint},
    views::{LinearLayout, PaddedView, Panel, ProgressBar, TextView, ThemedView, ViewRef},
    CbSink, Cursive, CursiveRunnable,
};
use gstreamer::State as GstState;
use tokio::select;

pub struct CursiveUI<'c> {
    root: CursiveRunnable,
    controls: &'c Controls,
}

impl<'c> CursiveUI<'c> {
    pub fn new(controls: &'c Controls) -> Self {
        let siv = cursive::default();

        Self {
            root: siv,
            controls,
        }
    }

    pub async fn run(&mut self) {
        // let mut screens: ScreensView<Panel<PaddedView<LinearLayout>>> = ScreensView::new();

        // let mut my_playlists = LinearLayout::new(Orientation::Vertical);

        // let mut list = SelectView::new();

        // if let Ok(my_lists) = self.client.user_playlists().await {
        //     my_lists.playlists.items.iter().for_each(|playlist| {
        //         list.add_item(playlist.name.as_str(), playlist.id);
        //     });
        // }

        // my_playlists.add_child(list);

        // screens.add_active_screen(
        //     Panel::new(PaddedView::lrtb(2, 2, 1, 1, my_playlists)).title("Open Playlist"),
        // );
        //
        let mut track_info = LinearLayout::new(Orientation::Horizontal)
            .resized(SizeConstraint::Full, SizeConstraint::Free);

        let mut meta = LinearLayout::new(Orientation::Vertical);

        let mut track_num = LinearLayout::new(Orientation::Vertical).fixed_width(4);

        meta.add_child(
            TextView::new("")
                .style(Effect::Bold)
                .with_name("track_title"),
        );

        meta.add_child(TextView::new("").with_name("album_title"));
        meta.add_child(TextView::new("").with_name("album_name"));
        meta.add_child(TextView::new("").with_name("album_release_date"));

        let track_num_mut = track_num.get_inner_mut();

        track_num_mut.add_child(
            TextView::new("")
                .h_align(HAlign::Center)
                .with_name("current_track_number"),
        );
        track_num_mut.add_child(TextView::new("of").h_align(HAlign::Center));
        track_num_mut.add_child(
            TextView::new("")
                .h_align(HAlign::Center)
                .with_name("total_tracks"),
        );

        let counter = Counter::new(0);
        let progress = ProgressBar::new()
            .with_value(counter)
            .range(0, 1)
            .with_name("progress")
            .full_width();

        meta.add_child(progress);

        let track_info_inner = track_info.get_inner_mut();

        track_info_inner.add_child(track_num);
        track_info_inner.add_child(meta);

        track_info_inner.add_child(TextView::new("null").with_name("player_status"));

        let player = Panel::new(
            PaddedView::lrtb(1, 1, 0, 0, track_info)
                .resized(SizeConstraint::Full, SizeConstraint::Full),
        )
        .title("hifi.rs");

        let theme = Theme {
            shadow: false,
            ..Default::default()
        };

        self.root.add_global_callback('q', Cursive::quit);
        let c = self.controls.clone();
        self.root.add_global_callback('p', move |_| {
            c.play_pause_blocking();
        });
        self.root
            .add_fullscreen_layer(ThemedView::new(theme, player));

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
                            let mut view: ViewRef<TextView> = s.find_name("player_status").unwrap();

                            match status.into() {
                                GstState::Playing => {
                                    view.set_content("playing");
                                }
                                GstState::Paused => {
                                    view.set_content("paused");
                                }
                                _ => {}
                            }
                        })).expect("failed to send update");

                    }
                    Notification::Position {position} => {

                    }
                    Notification::Buffering => {
                        cb.send(Box::new(|s| {
                            let mut view: ViewRef<TextView> = s.find_name("player_status").unwrap();
                            view.set_content("buffering");
                        })).expect("failed to send update");
                    },

                }
            }
        }
    }
}
