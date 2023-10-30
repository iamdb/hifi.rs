use crate::{
    player::{self, controls::Controls, notification::Notification, queue::TrackListValue},
    service::{Album, Track},
};
use chrono::{DateTime, Duration, Local};
use gstreamer::{ClockTime, State as GstState};
use std::collections::HashMap;
use zbus::{dbus_interface, fdo::Result, zvariant, Connection, ConnectionBuilder, SignalContext};

#[derive(Debug)]
pub struct Mpris {
    controls: Controls,
}

pub async fn init(controls: Controls) -> Connection {
    let mpris = Mpris {
        controls: controls.clone(),
    };
    let mpris_player = MprisPlayer {
        controls: controls.clone(),
        status: GstState::Null,
        current_track: None,
        total_tracks: 0,
        position: ClockTime::default(),
        position_ts: chrono::offset::Local::now(),
        can_play: true,
        can_pause: true,
        can_stop: true,
        can_next: true,
        can_previous: true,
    };
    let mpris_tracklist = MprisTrackList {
        controls,
        track_list: TrackListValue::new(None),
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

pub async fn receive_notifications(conn: Connection) {
    let mut receiver = player::notify_receiver();
    let object_server = conn.object_server();

    loop {
        if let Ok(notification) = receiver.recv().await {
            match notification {
                Notification::Quit => {
                    return;
                }
                Notification::Loading {
                    is_loading: _,
                    target_state: _,
                } => {}
                Notification::Buffering {
                    is_buffering: _,
                    target_state: _,
                    percent: _,
                } => {
                    let iface_ref = object_server
                        .interface::<_, MprisPlayer>("/org/mpris/MediaPlayer2")
                        .await
                        .expect("failed to get object server");

                    iface_ref
                        .get_mut()
                        .await
                        .playback_status_changed(iface_ref.signal_context())
                        .await
                        .expect("failed to signal metadata change");
                }
                Notification::Status { status } => {
                    let iface_ref = object_server
                        .interface::<_, MprisPlayer>("/org/mpris/MediaPlayer2")
                        .await
                        .expect("failed to get object server");

                    let mut iface = iface_ref.get_mut().await;
                    iface.status = status;

                    match status {
                        GstState::Null => {
                            iface.can_play = true;
                            iface.can_pause = true;
                            iface.can_stop = false;
                        }
                        GstState::Paused => {
                            iface.can_play = true;
                            iface.can_pause = false;
                            iface.can_stop = true;
                        }
                        GstState::Playing => {
                            iface.position_ts = chrono::offset::Local::now();
                            iface.can_play = true;
                            iface.can_pause = true;
                            iface.can_stop = true;
                        }
                        _ => {
                            iface.can_play = true;
                            iface.can_pause = true;
                            iface.can_stop = true;
                        }
                    }

                    iface
                        .playback_status_changed(iface_ref.signal_context())
                        .await
                        .expect("failed to signal metadata change");
                }
                Notification::Position { clock } => {
                    let iface_ref = object_server
                        .interface::<_, MprisPlayer>("/org/mpris/MediaPlayer2")
                        .await
                        .expect("failed to get object server");

                    let mut iface = iface_ref.get_mut().await;
                    let now = chrono::offset::Local::now();
                    let diff = now.signed_duration_since(iface.position_ts);
                    let position_secs = clock.seconds();

                    if diff.num_seconds() != position_secs as i64 {
                        debug!("mpris clock drift, sending new position");
                        iface.position_ts =
                            chrono::offset::Local::now() - Duration::seconds(position_secs as i64);

                        MprisPlayer::seeked(iface_ref.signal_context(), clock.useconds() as i64)
                            .await
                            .expect("failed to send seeked signal");
                    }
                }
                Notification::CurrentTrackList { list } => {
                    if let Some(current) = list.current_track() {
                        let player_ref = object_server
                            .interface::<_, MprisPlayer>("/org/mpris/MediaPlayer2")
                            .await
                            .expect("failed to get object server");

                        let mut player_iface = player_ref.get_mut().await;

                        player_iface.can_previous = current.position != 0;

                        player_iface.can_next = !(player_iface.total_tracks != 0
                            && current.position == player_iface.total_tracks - 1);

                        if let Some(album) = &current.album {
                            player_iface.total_tracks = album.total_tracks;
                        }

                        player_iface.current_track = Some(current.clone());
                        player_iface.current_album = list.get_album().cloned();

                        player_iface
                            .metadata_changed(player_ref.signal_context())
                            .await
                            .expect("failed to signal metadata change");

                        let list_ref = object_server
                            .interface::<_, MprisTrackList>("/org/mpris/MediaPlayer2")
                            .await
                            .expect("failed to get object server");

                        let tracks = list
                            .cursive_list()
                            .iter()
                            .map(|t| t.0.clone())
                            .collect::<Vec<String>>();
                        let mut list_iface = list_ref.get_mut().await;

                        MprisTrackList::track_list_replaced(
                            list_ref.signal_context(),
                            tracks,
                            &current.title,
                        )
                        .await
                        .expect("failed to send track list replaced signal");

                        list_iface.track_list = list;
                    }
                }
                Notification::Error { error: _ } => {}
                Notification::AudioQuality {
                    bitdepth: _,
                    sampling_rate: _,
                } => {}
            }
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
    status: GstState,
    position: ClockTime,
    position_ts: DateTime<Local>,
    total_tracks: u32,
    current_track: Option<Track>,
    can_play: bool,
    can_pause: bool,
    can_stop: bool,
    can_next: bool,
    can_previous: bool,
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
        match self.status {
            GstState::Playing => "Playing".to_string(),
            GstState::Paused => "Paused".to_string(),
            GstState::Null => "Stopped".to_string(),
            GstState::VoidPending => "Stopped".to_string(),
            GstState::Ready => "Ready".to_string(),
        }
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
        debug!("signal metadata refresh");
        if let Some(current_track) = &self.current_track {
            track_to_meta(current_track, current_track.album.as_ref())
        } else {
            HashMap::default()
        }
    }
    #[dbus_interface(property, name = "Volume")]
    fn volume(&self) -> f64 {
        1.0
    }
    #[dbus_interface(property, name = "Position")]
    async fn position(&self) -> i64 {
        self.position.useconds() as i64
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
        self.can_next
    }
    #[dbus_interface(property, name = "CanGoPrevious")]
    fn can_go_previous(&self) -> bool {
        self.can_previous
    }
    #[dbus_interface(property, name = "CanPlay")]
    fn can_play(&self) -> bool {
        self.can_play
    }
    #[dbus_interface(property, name = "CanPause")]
    fn can_pause(&self) -> bool {
        self.can_pause
    }
    #[dbus_interface(property, name = "CanStop")]
    fn can_stop(&self) -> bool {
        self.can_stop
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
    track_list: TrackListValue,
}

#[dbus_interface(name = "org.mpris.MediaPlayer2.TrackList")]
impl MprisTrackList {
    async fn get_tracks_metadata(
        &self,
        tracks: Vec<String>,
    ) -> Vec<HashMap<&'static str, zvariant::Value>> {
        debug!("get tracks metadata");

        self.track_list
            .unplayed_tracks()
            .into_iter()
            .filter_map(|i| {
                if tracks.contains(&i.position.to_string()) {
                    Some(track_to_meta(i, self.track_list.get_album()))
                } else {
                    None
                }
            })
            .collect::<Vec<HashMap<&'static str, zvariant::Value>>>()
    }

    async fn go_to(&self, position: String) {
        if let Ok(p) = position.parse::<u32>() {
            self.controls.skip_to(p).await;
        }
    }

    #[dbus_interface(signal, name = "TrackListReplaced")]
    pub async fn track_list_replaced(
        #[zbus(signal_context)] ctxt: &SignalContext<'_>,
        tracks: Vec<String>,
        current: &str,
    ) -> zbus::Result<()>;

    #[dbus_interface(property, name = "Tracks")]
    async fn tracks(&self) -> Vec<String> {
        self.track_list
            .unplayed_tracks()
            .iter()
            .map(|i| i.position.to_string())
            .collect::<Vec<String>>()
    }

    #[dbus_interface(property, name = "CanEditTracks")]
    async fn can_edit_tracks(&self) -> bool {
        false
    }
}

fn track_to_meta(
    playlist_track: &Track,
    album: Option<&Album>,
) -> HashMap<&'static str, zvariant::Value<'static>> {
    let mut meta = HashMap::new();

    meta.insert(
        "mpris:trackid",
        zvariant::Value::new(format!(
            "/org/hifirs/Player/TrackList/{}",
            playlist_track.id
        )),
    );
    meta.insert(
        "xesam:title",
        zvariant::Value::new(playlist_track.title.trim().to_string()),
    );
    meta.insert(
        "xesam:trackNumber",
        zvariant::Value::new(playlist_track.position as i32),
    );

    meta.insert(
        "mpris:length",
        zvariant::Value::new(
            ClockTime::from_seconds(playlist_track.duration_seconds as u64).useconds() as i64,
        ),
    );

    if let Some(artist) = &playlist_track.artist {
        meta.insert(
            "xesam:artist",
            zvariant::Value::new(artist.name.trim().to_string()),
        );
    }

    if let Some(album) = album {
        meta.insert(
            "mpris:artUrl",
            zvariant::Value::new(album.cover_art.clone()),
        );
        meta.insert(
            "xesam:album",
            zvariant::Value::new(album.title.trim().to_string()),
        );
        meta.insert(
            "xesam:albumArtist",
            zvariant::Value::new(album.artist.name.trim().to_string()),
        );
        meta.insert(
            "xesam:artist",
            zvariant::Value::new(album.artist.name.trim().to_string()),
        );
    }

    meta
}
