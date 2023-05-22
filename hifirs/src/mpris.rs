use crate::{player::Controls, state::app::SafePlayerState};
use gstreamer::ClockTime;
use hifirs_qobuz_api::client::track::TrackListTrack;
use std::collections::HashMap;
use zbus::{dbus_interface, fdo::Result, zvariant, Connection, ConnectionBuilder, SignalContext};

#[derive(Debug)]
pub struct Mpris {
    controls: Controls,
}

pub async fn init(state: SafePlayerState, controls: &Controls) -> Connection {
    let mpris = Mpris {
        controls: controls.clone(),
    };
    let mpris_player = MprisPlayer {
        controls: controls.clone(),
        state: state.clone(),
    };
    let mpris_tracklist = MprisTrackList {
        controls: controls.clone(),
        state,
    };

    let conn = ConnectionBuilder::session()
        .unwrap()
        .serve_at("/org/mpris/MediaPlayer2", mpris)
        .unwrap()
        .serve_at("/org/mpris/MediaPlayer2", mpris_player)
        .unwrap()
        .serve_at("/org/mpris/MediaPlayer2", mpris_tracklist)
        .unwrap()
        .name("org.mpris.MediaPlayer2.hifirs")
        .unwrap()
        .internal_executor(false)
        .build()
        .await;

    match conn {
        Ok(c) => c,
        Err(err) => {
            println!("There was an error with mpris and I must exit.");
            println!("Message: {err}");
            std::process::exit(1);
        }
    }
}

#[dbus_interface(name = "org.mpris.MediaPlayer2")]
impl Mpris {
    async fn quit(&self) -> Result<()> {
        self.controls.quit().await;
        Ok(())
    }

    #[dbus_interface(property, name = "CanQuit")]
    fn can_quit(&self) -> bool {
        true
    }
    #[dbus_interface(property, name = "CanSetFullscreen")]
    fn can_set_fullscreen(&self) -> bool {
        false
    }
    #[dbus_interface(property, name = "CanRaise")]
    fn can_raise(&self) -> bool {
        false
    }
    #[dbus_interface(property, name = "SupportedMimeTypes")]
    fn supported_mime_types(&self) -> Vec<&'static str> {
        vec!["audio/mpeg", "audio/x-flac", "audio/flac"]
    }
    #[dbus_interface(property, name = "SupportedUriSchemes")]
    fn supported_uri_schemes(&self) -> Vec<&'static str> {
        vec!["http"]
    }
    #[dbus_interface(property)]
    fn identity(&self) -> &'static str {
        "hifi-rs"
    }
    #[dbus_interface(property)]
    fn has_track_list(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct MprisPlayer {
    controls: Controls,
    state: SafePlayerState,
}

#[dbus_interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {
    async fn open_uri(&self, uri: String) {
        self.controls.play_uri(uri).await;
    }
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
    #[dbus_interface(property, name = "PlaybackStatus")]
    async fn playback_status(&self) -> String {
        self.state.read().await.status().as_str().to_string()
    }
    #[dbus_interface(property, name = "LoopStatus")]
    fn loop_status(&self) -> &'static str {
        "None"
    }
    #[dbus_interface(property, name = "Rate")]
    fn rate(&self) -> f64 {
        1.0
    }
    #[dbus_interface(property, name = "Shuffle")]
    fn shuffle(&self) -> bool {
        false
    }
    #[dbus_interface(property, name = "Metadata")]
    async fn metadata(&self) -> HashMap<&'static str, zvariant::Value> {
        debug!("dbus metadata");
        if let Some(next_up) = self.state.read().await.current_track() {
            track_to_meta(next_up)
        } else {
            HashMap::new()
        }
    }
    #[dbus_interface(property, name = "Volume")]
    fn volume(&self) -> f64 {
        1.0
    }
    #[dbus_interface(property, name = "Position")]
    async fn position(&self) -> i64 {
        self.state
            .read()
            .await
            .position()
            .inner_clocktime()
            .useconds() as i64
    }
    #[dbus_interface(signal, name = "Seeked")]
    pub async fn seeked(
        #[zbus(signal_context)] ctxt: &SignalContext<'_>,
        message: i64,
    ) -> zbus::Result<()>;
    #[dbus_interface(property, name = "MinimumRate")]
    fn minimum_rate(&self) -> f64 {
        1.0
    }
    #[dbus_interface(property, name = "MaxiumumRate")]
    fn maximum_rate(&self) -> f64 {
        1.0
    }
    #[dbus_interface(property, name = "CanGoNext")]
    fn can_go_next(&self) -> bool {
        true
    }
    #[dbus_interface(property, name = "CanGoPrevious")]
    fn can_go_previous(&self) -> bool {
        true
    }
    #[dbus_interface(property, name = "CanPlay")]
    fn can_play(&self) -> bool {
        true
    }
    #[dbus_interface(property, name = "CanPause")]
    fn can_pause(&self) -> bool {
        true
    }
    #[dbus_interface(property, name = "CanSeek")]
    fn can_seek(&self) -> bool {
        true
    }
    #[dbus_interface(property, name = "CanControl")]
    fn can_control(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct MprisTrackList {
    controls: Controls,
    state: SafePlayerState,
}

#[dbus_interface(name = "org.mpris.MediaPlayer2.TrackList")]
impl MprisTrackList {
    async fn get_tracks_metadata(
        &self,
        tracks: Vec<String>,
    ) -> Vec<HashMap<&'static str, zvariant::Value>> {
        self.state
            .read()
            .await
            .unplayed_tracks()
            .into_iter()
            .filter_map(|i| {
                if tracks.contains(&i.track.id.to_string()) {
                    Some(track_to_meta(i.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<HashMap<&'static str, zvariant::Value>>>()
    }
    async fn go_to(&self, track_id: String) {
        if let Ok(id) = track_id.parse::<usize>() {
            self.controls.skip_to_by_id(id).await;
        }
    }
    #[dbus_interface(signal, name = "Seeked")]
    pub async fn track_list_replaced(
        #[zbus(signal_context)] ctxt: &SignalContext<'_>,
        tracks: Vec<String>,
        current: String,
    ) -> zbus::Result<()>;
    #[dbus_interface(property, name = "Tracks")]
    async fn tracks(&self) -> Vec<String> {
        self.state
            .read()
            .await
            .unplayed_tracks()
            .iter()
            .map(|i| i.track.id.to_string())
            .collect::<Vec<String>>()
    }
    #[dbus_interface(property, name = "CanEditTracks")]
    async fn can_edit_tracks(&self) -> bool {
        false
    }
}

fn track_to_meta(
    playlist_track: TrackListTrack,
) -> HashMap<&'static str, zvariant::Value<'static>> {
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

    if let Some(artist) = playlist_track.track.performer {
        meta.insert("xesam:artist", zvariant::Value::new(artist.name));
    }

    if let Some(album) = playlist_track.album {
        meta.insert("mpris:artUrl", zvariant::Value::new(album.image.large));
        meta.insert("xesam:album", zvariant::Value::new(album.title));
        meta.insert(
            "xesam:albumArtist",
            zvariant::Value::new(album.artist.name.clone()),
        );
        meta.insert("xesam:artist", zvariant::Value::new(album.artist.name));
    }

    meta
}
