use crate::{
    player::{self, notification::Notification},
    service::{Album, Track},
};
use chrono::{DateTime, Duration, Local};
use futures::executor::block_on;
use gstreamer::{ClockTime, State as GstState};
use std::collections::HashMap;
use zbus::{dbus_interface, fdo::Result, zvariant, Connection, ConnectionBuilder, SignalContext};

#[derive(Debug)]
pub struct Mpris {}

pub async fn init() -> Connection {
    let mpris = Mpris {};
    let mpris_player = MprisPlayer {
        status: GstState::Null,
        total_tracks: 0,
        position: ClockTime::default(),
        position_ts: chrono::offset::Local::now(),
        can_play: true,
        can_pause: true,
        can_stop: true,
        can_next: true,
        can_previous: true,
    };
    let mpris_tracklist = MprisTrackList {};

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

pub async fn receive_notifications(conn: &Connection) {
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
                    let player_ref = object_server
                        .interface::<_, MprisPlayer>("/org/mpris/MediaPlayer2")
                        .await
                        .expect("failed to get object server");

                    let list_ref = object_server
                        .interface::<_, MprisTrackList>("/org/mpris/MediaPlayer2")
                        .await
                        .expect("failed to get object server");

                    let mut player_iface = player_ref.get_mut().await;

                    if let Some(album) = list.get_album() {
                        player_iface.total_tracks = album.total_tracks;
                    }

                    if let Some(current) = list.current_track() {
                        player_iface.can_previous = current.position != 0;

                        player_iface.can_next = !(player_iface.total_tracks != 0
                            && current.position == player_iface.total_tracks - 1);

                        let tracks = list
                            .cursive_list()
                            .iter()
                            .map(|t| t.0)
                            .collect::<Vec<&str>>();

                        MprisTrackList::track_list_replaced(
                            list_ref.signal_context(),
                            tracks,
                            &current.title,
                        )
                        .await
                        .expect("failed to send track list replaced signal");
                    }

                    player_iface
                        .metadata_changed(player_ref.signal_context())
                        .await
                        .expect("failed to signal metadata change");
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
        if let Err(error) = player::quit().await {
            debug!(?error);
        }

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
    fn supported_mime_types(&self) -> Vec<&str> {
        vec!["audio/mpeg", "audio/x-flac", "audio/flac"]
    }
    #[dbus_interface(property, name = "SupportedUriSchemes")]
    fn supported_uri_schemes(&self) -> Vec<&str> {
        vec!["http"]
    }
    #[dbus_interface(property)]
    fn identity(&self) -> &str {
        "hifi-rs"
    }
    #[dbus_interface(property)]
    fn has_track_list(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct MprisPlayer {
    status: GstState,
    position: ClockTime,
    position_ts: DateTime<Local>,
    total_tracks: u32,
    can_play: bool,
    can_pause: bool,
    can_stop: bool,
    can_next: bool,
    can_previous: bool,
}

#[dbus_interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {
    async fn open_uri(&self, uri: &str) {
        if let Err(error) = player::play_uri(uri).await {
            debug!(?error);
        }
    }
    async fn play(&self) {
        if let Err(error) = player::play().await {
            debug!(?error);
        }
    }
    async fn pause(&self) {
        if let Err(error) = player::pause().await {
            debug!(?error);
        }
    }
    async fn stop(&self) {
        if let Err(error) = player::stop().await {
            debug!(?error);
        }
    }
    async fn play_pause(&self) {
        if let Err(error) = player::play_pause().await {
            debug!(?error);
        }
    }
    async fn next(&self) {
        if let Err(error) = player::next().await {
            debug!(?error);
        }
    }
    async fn previous(&self) {
        if let Err(error) = player::previous().await {
            debug!(?error);
        }
    }
    #[dbus_interface(property, name = "PlaybackStatus")]
    async fn playback_status(&self) -> &str {
        match self.status {
            GstState::Playing => "Playing",
            GstState::Paused => "Paused",
            GstState::Null => "Stopped",
            GstState::VoidPending => "Stopped",
            GstState::Ready => "Ready",
        }
    }
    #[dbus_interface(property, name = "LoopStatus")]
    fn loop_status(&self) -> &str {
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
    async fn metadata(&self) -> HashMap<&str, zvariant::Value> {
        debug!("signal metadata refresh");
        if let Some(current_track) = player::current_track().await {
            track_to_meta(
                current_track,
                player::current_tracklist().await.get_album().cloned(),
            )
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
pub struct MprisTrackList {}

#[dbus_interface(name = "org.mpris.MediaPlayer2.TrackList")]
impl MprisTrackList {
    async fn get_tracks_metadata(
        &self,
        tracks: Vec<String>,
    ) -> Vec<HashMap<&str, zvariant::Value>> {
        debug!("get tracks metadata");

        player::current_tracklist()
            .await
            .all_tracks()
            .into_iter()
            .filter_map(|i| {
                if tracks.contains(&i.position.to_string()) {
                    let album =
                        block_on(async { player::current_tracklist().await.get_album().cloned() });
                    Some(track_to_meta(i.clone(), album))
                } else {
                    None
                }
            })
            .collect::<Vec<HashMap<&str, zvariant::Value>>>()
    }

    async fn go_to(&self, position: String) {
        if let Ok(p) = position.parse::<u32>() {
            if let Err(error) = player::skip(p, true).await {
                debug!(?error);
            }
        }
    }

    #[dbus_interface(signal, name = "TrackListReplaced")]
    pub async fn track_list_replaced(
        #[zbus(signal_context)] ctxt: &SignalContext<'_>,
        tracks: Vec<&str>,
        current: &str,
    ) -> zbus::Result<()>;

    #[dbus_interface(property, name = "Tracks")]
    async fn tracks(&self) -> Vec<String> {
        player::current_tracklist()
            .await
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

fn track_to_meta<'a>(
    playlist_track: Track,
    album: Option<Album>,
) -> HashMap<&'a str, zvariant::Value<'a>> {
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
