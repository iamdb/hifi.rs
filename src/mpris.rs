use std::collections::HashMap;

use crate::{player::Controls, qobuz::track::PlaylistTrack};
use gstreamer::ClockTime;
use zbus::{dbus_interface, fdo::Result, zvariant, ConnectionBuilder};

#[derive(Debug)]
pub struct Mpris {
    controls: Controls,
}

pub async fn init(controls: Controls) {
    let mpris = Mpris {
        controls: controls.clone(),
    };
    let mpris_player = MprisPlayer {
        controls: controls.clone(),
    };
    let mpris_tracklist = MprisTrackList { controls };

    if let Err(err) = ConnectionBuilder::session()
        .unwrap()
        .name("org.mpris.MediaPlayer2.hifirs")
        .unwrap()
        .serve_at("/org/mpris/MediaPlayer2", mpris)
        .unwrap()
        .serve_at("/org/mpris/MediaPlayer2", mpris_player)
        .unwrap()
        .serve_at("/org/mpris/MediaPlayer2", mpris_tracklist)
        .unwrap()
        .build()
        .await
    {
        error!("error connecting to dbus: {}", err);
    }
}

#[dbus_interface(name = "org.mpris.MediaPlayer2")]
impl Mpris {
    async fn quit(&self) -> Result<()> {
        self.controls.stop().await;
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

#[derive(Debug)]
pub struct MprisPlayer {
    controls: Controls,
}

#[dbus_interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {
    async fn play(&self) {
        self.controls.play().await;
    }
    async fn pause(&self) {
        self.controls.pause().await;
    }
    async fn stop(&self) {
        self.controls.stop().await;
    }
    async fn play_pause(&self) {
        self.controls.play_pause().await;
    }
    async fn next(&self) {
        self.controls.next().await;
    }
    async fn previous(&self) {
        self.controls.previous().await;
    }
    #[dbus_interface(property)]
    async fn playback_status(&self) -> &'static str {
        if let Some(status) = self.controls.status().await {
            status.as_str()
        } else {
            ""
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
    async fn metadata(&self) -> HashMap<&'static str, zvariant::Value> {
        if let Some(next_up) = self.controls.currently_playing_track().await {
            track_to_meta(next_up)
        } else {
            HashMap::new()
        }
    }
    #[dbus_interface(property)]
    fn volume(&self) -> f64 {
        1.0
    }
    #[dbus_interface(property)]
    async fn position(&self) -> i64 {
        if let Some(position) = self.controls.position().await {
            position.inner_clocktime().useconds() as i64
        } else {
            0
        }
    }
    // #[dbus_interface(property)]
    // fn set_position(&self) {
    //     self.player.seek();
    // }
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

#[derive(Debug)]
pub struct MprisTrackList {
    controls: Controls,
}

#[dbus_interface(name = "org.mpris.MediaPlayer2.TrackList")]
impl MprisTrackList {
    async fn get_tracks_metadata(
        &self,
        tracks: Vec<String>,
    ) -> Vec<HashMap<&'static str, zvariant::Value>> {
        if let Some(playlist) = self.controls.remaining_tracks().await {
            playlist
                .vec()
                .iter()
                .filter_map(|i| {
                    if tracks.contains(&i.track.id.to_string()) {
                        Some(track_to_meta(i.clone()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<HashMap<&'static str, zvariant::Value>>>()
        } else {
            vec![]
        }
    }
    async fn go_to(&self, track_id: String) {
        if let Ok(id) = track_id.parse::<usize>() {
            self.controls.skip_to_by_id(id).await;
        }
    }
    #[dbus_interface(property)]
    async fn tracks(&self) -> Vec<String> {
        if let Some(playlist) = self.controls.remaining_tracks().await {
            playlist
                .vec()
                .iter()
                .map(|i| i.track.id.to_string())
                .collect::<Vec<String>>()
        } else {
            vec![]
        }
    }
    #[dbus_interface(property)]
    async fn can_edit_tracks(&self) -> bool {
        false
    }
}

fn track_to_meta(playlist_track: PlaylistTrack) -> HashMap<&'static str, zvariant::Value<'static>> {
    let mut meta = HashMap::new();

    meta.insert(
        "mpris:trackid",
        zvariant::Value::new(format!(
            "/org/hifirs/Player/TrackList/{}",
            playlist_track.track.id
        )),
    );
    meta.insert(
        "xesam:title",
        zvariant::Value::new(playlist_track.track.title),
    );
    meta.insert(
        "xesam:trackNumber",
        zvariant::Value::new(playlist_track.track.track_number),
    );

    meta.insert(
        "mpris:length",
        zvariant::Value::new(
            ClockTime::from_seconds(playlist_track.track.duration as u64).useconds() as i64,
        ),
    );

    if let Some(album) = playlist_track.album {
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

    meta
}
