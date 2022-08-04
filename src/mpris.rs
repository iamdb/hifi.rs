use std::collections::HashMap;

use crate::{
    get_player,
    player::Player,
    qobuz::PlaylistTrack,
    state::app::{AppKey, PlayerKey},
};
use gst::State as GstState;
use gstreamer as gst;
use zbus::{dbus_interface, fdo::Result, zvariant, ConnectionBuilder};

pub struct Mpris {
    player: Player,
}

pub async fn init(player: Player) {
    let mpris = Mpris {
        player: player.clone(),
    };
    let mpris_player = MprisPlayer { player };

    ConnectionBuilder::session()
        .unwrap()
        .name("org.mpris.MediaPlayer2.hifirs")
        .unwrap()
        .serve_at("/org/mpris/MediaPlayer2", mpris)
        .unwrap()
        .serve_at("/org/mpris/MediaPlayer2", mpris_player)
        .unwrap()
        .build()
        .await
        .expect("failed to attach to dbus");
}

#[dbus_interface(name = "org.mpris.MediaPlayer2")]
impl Mpris {
    fn quit(&self) -> Result<()> {
        self.player.stop();
        Ok(())
    }
    #[dbus_interface(property)]
    fn can_quit(&self) -> bool {
        true
    }
    #[dbus_interface(property)]
    fn can_set_fullscreen(&self) -> bool {
        false
    }
    #[dbus_interface(property)]
    fn can_raise(&self) -> bool {
        false
    }
    #[dbus_interface(property)]
    fn supported_mime_types(&self) -> Vec<&'static str> {
        vec!["audio/mpeg", "audio/x-flac", "audio/flac"]
    }
    #[dbus_interface(property)]
    fn supported_uri_schemes(&self) -> Vec<&'static str> {
        vec!["http"]
    }
    #[dbus_interface(property)]
    fn identity(&self) -> &'static str {
        "hifi-rs"
    }
}

pub struct MprisPlayer {
    player: Player,
}

#[dbus_interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {
    fn play(&self) {
        self.player.play();
    }
    fn pause(&self) {
        self.player.pause();
    }
    fn stop(&self) {
        self.player.stop();
    }
    fn play_pause(&self) {
        if self.player.is_playing() {
            self.player.pause();
        } else if self.player.is_paused() {
            self.player.play()
        }
    }
    fn next(&mut self) {
        self.player
            .skip_forward(None)
            .expect("failed to skip forward");
    }
    fn previous(&mut self) {
        self.player
            .skip_backward(None)
            .expect("failed to to skip backward");
    }
    fn seek(&self) -> Result<()> {
        Ok(())
    }
    #[dbus_interface(property)]
    fn playback_status(&self) -> &'static str {
        match self.player.current_state() {
            GstState::Playing => "Playing",
            GstState::Paused => "Paused",
            GstState::Null => "Stopped",
            GstState::VoidPending => "Stopped",
            GstState::Ready => "Stopped",
            _ => "",
        }
    }
    #[dbus_interface(property)]
    fn loop_status(&self) -> &'static str {
        "None"
    }
    #[dbus_interface(property)]
    fn rate(&self) -> f64 {
        1.0
    }
    #[dbus_interface(property)]
    fn shuffle(&self) -> bool {
        false
    }
    #[dbus_interface(property)]
    fn metadata(&self) -> HashMap<&'static str, zvariant::Value> {
        let mut meta = HashMap::new();
        let tree = self.player.app_state().player;

        if let Some(next_up) = get_player!(PlayerKey::NextUp, tree, PlaylistTrack) {
            meta.insert(
                "mpris:trackid",
                zvariant::Value::new(format!("/org/hifirs/Player/TrackList/{}", next_up.track.id)),
            );
            meta.insert(
                "mpris:length",
                zvariant::Value::new(self.player.duration().useconds() as i64),
            );
            meta.insert("xesam:title", zvariant::Value::new(next_up.track.title));
            meta.insert(
                "xesam:trackNumber",
                zvariant::Value::new(next_up.track.track_number),
            );

            if let Some(album) = next_up.album {
                if let Some(thumb) = album.image.thumbnail {
                    meta.insert("mpris:artUrl", zvariant::Value::new(thumb));
                }
                meta.insert("xesam:album", zvariant::Value::new(album.title));
                meta.insert(
                    "xesam:albumArtist",
                    zvariant::Value::new(album.artist.name.clone()),
                );
                meta.insert("xesam:artist", zvariant::Value::new(album.artist.name));
            }
        }

        meta
    }
    #[dbus_interface(property)]
    fn volume(&self) -> f64 {
        1.0
    }
    #[dbus_interface(property)]
    fn position(&self) -> i64 {
        let position = self.player.position();

        position.useconds() as i64
    }
    #[dbus_interface(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }
    #[dbus_interface(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }
    #[dbus_interface(property)]
    fn can_go_next(&self) -> bool {
        true
    }
    #[dbus_interface(property)]
    fn can_go_previous(&self) -> bool {
        true
    }
    #[dbus_interface(property)]
    fn can_play(&self) -> bool {
        true
    }
    #[dbus_interface(property)]
    fn can_pause(&self) -> bool {
        true
    }
    #[dbus_interface(property)]
    fn can_seek(&self) -> bool {
        true
    }
    #[dbus_interface(property)]
    fn can_control(&self) -> bool {
        true
    }
}
