use crate::{
    player::{
        controls::{Action, Controls},
        error::Error,
        notification::{BroadcastReceiver, BroadcastSender, Notification},
        queue::{
            controls::{PlayerState, SafePlayerState, SkipDirection},
            TrackListValue,
        },
    },
    service::{Album, Playlist, SearchResults, Track},
    REFRESH_RESOLUTION,
};
use cached::proc_macro::cached;
use flume::{Receiver, Sender};
use futures::prelude::*;
use gst::{
    prelude::*, Caps, ClockTime, Element, Message, MessageType, MessageView, SeekFlags,
    State as GstState, StateChangeSuccess, Structure,
};
use gstreamer as gst;
use hifirs_qobuz_api::client::{self, UrlType};
use once_cell::sync::{Lazy, OnceCell};
use std::{
    str::FromStr,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{select, sync::RwLock};

#[macro_use]
pub mod controls;
pub mod error;
pub mod notification;
#[macro_use]
pub mod queue;

pub type Result<T, E = Error> = std::result::Result<T, E>;

static VERSION: Lazy<(u32, u32, u32, u32)> = Lazy::new(gstreamer::version);

static PLAYBIN: Lazy<Element> = Lazy::new(|| {
    gst::init().expect("error initializing gstreamer");

    let playbin = gst::ElementFactory::make("playbin3")
        .build()
        .expect("error building playbin element");

    playbin.set_property_from_str("flags", "audio+buffering");
    if VERSION.1 >= 22 {
        playbin.connect("element-setup", false, |value| {
            let element = &value[1].get::<gst::Element>().unwrap();

            if element.name().contains("urisourcebin") {
                element.set_property("parse-streams", true);
            }

            None
        });
    }
    playbin.connect("source-setup", false, |value| {
        let element = &value[1].get::<gst::Element>().unwrap();

        if element.name().contains("souphttpsrc") {
            debug!("new source, changing settings");
            let ua = if rand::random() {
                USER_AGENTS[0]
            } else {
                USER_AGENTS[1]
            };
            element.set_property("user-agent", ua);
            element.set_property("compress", true);
            element.set_property("retries", 10);
            element.set_property("timeout", 30_u32);
            element.set_property(
                "extra-headers",
                Structure::from_str("a-structure, DNT=1, Pragma=no-cache, Cache-Control=no-cache")
                    .expect("failed to make structure from string"),
            )
        }

        None
    });
    playbin.add_property_deep_notify_watch(Some("caps"), true);

    // Connects to the `about-to-finish` signal so the player
    // can setup the next track to play. Enables gapless playback.
    playbin.connect("about-to-finish", false, move |_| {
        debug!("about to finish");
        ABOUT_TO_FINISH
            .tx
            .send(true)
            .expect("failed to send about to finish message");

        None
    });

    playbin
});
static CONTROLS: Lazy<Controls> = Lazy::new(Controls::new);

struct Broadcast {
    tx: BroadcastSender,
    rx: BroadcastReceiver,
}

static BROADCAST_CHANNELS: Lazy<Broadcast> = Lazy::new(|| {
    let (mut tx, rx) = async_broadcast::broadcast(20);
    tx.set_overflow(true);

    Broadcast { rx, tx }
});

struct AboutToFinish {
    tx: Sender<bool>,
    rx: Receiver<bool>,
}

static ABOUT_TO_FINISH: Lazy<AboutToFinish> = Lazy::new(|| {
    let (tx, rx) = flume::bounded::<bool>(1);

    AboutToFinish { tx, rx }
});
static QUIT_WHEN_DONE: AtomicBool = AtomicBool::new(false);
static IS_BUFFERING: AtomicBool = AtomicBool::new(false);
static IS_LIVE: AtomicBool = AtomicBool::new(false);
static SAMPLING_RATE: AtomicU32 = AtomicU32::new(44100);
static BIT_DEPTH: AtomicU32 = AtomicU32::new(16);
static QUEUE: OnceCell<SafePlayerState> = OnceCell::new();
static USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_4) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36"
];

#[instrument]
pub async fn init(
    username: Option<&str>,
    password: Option<&str>,
    quit_when_done: bool,
) -> Result<()> {
    let state = Arc::new(RwLock::new(PlayerState::new(username, password).await));
    let version = gstreamer::version();
    debug!(?version);

    QUEUE.set(state).expect("error setting player state");
    QUIT_WHEN_DONE.store(quit_when_done, Ordering::Relaxed);

    Ok(())
}
#[instrument]
/// Play the player.
pub async fn play() -> Result<()> {
    set_player_state(gst::State::Playing).await?;
    Ok(())
}
#[instrument]
/// Pause the player.
pub async fn pause() -> Result<()> {
    set_player_state(gst::State::Paused).await?;
    Ok(())
}
#[instrument]
/// Ready the player.
pub async fn ready() -> Result<()> {
    set_player_state(gst::State::Ready).await?;
    Ok(())
}
#[instrument]
/// Stop the player.
pub async fn stop() -> Result<()> {
    set_player_state(gst::State::Null).await?;
    Ok(())
}
#[instrument]
/// Sets the player to a specific state.
pub async fn set_player_state(state: gst::State) -> Result<()> {
    let ret = PLAYBIN.set_state(state)?;

    match ret {
        StateChangeSuccess::Success => {
            debug!("*** successful state change ***");
        }
        StateChangeSuccess::Async => {
            debug!("*** async state change ***");
        }
        StateChangeSuccess::NoPreroll => {
            debug!("*** stream is live ***");
            IS_LIVE.store(true, Ordering::Relaxed);
        }
    }

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    while current_state() != state {
        debug!("waiting for player to change state");
        interval.tick().await;
    }

    Ok(())
}
#[instrument]
/// Toggle play and pause.
pub async fn play_pause() -> Result<()> {
    let mut state = QUEUE.get().unwrap().write().await;

    if is_playing() {
        state.set_target_status(GstState::Paused);
        pause().await?;
    } else if is_paused() || is_ready() {
        state.set_target_status(GstState::Playing);
        play().await?;
    }

    Ok(())
}
#[instrument]
/// Is the player paused?
pub fn is_paused() -> bool {
    PLAYBIN.current_state() == gst::State::Paused
}
#[instrument]
/// Is the player playing?
pub fn is_playing() -> bool {
    PLAYBIN.current_state() == gst::State::Playing
}
#[instrument]
/// Is the player ready?
pub fn is_ready() -> bool {
    PLAYBIN.current_state() == gst::State::Ready
}
#[instrument]
/// Current player state
pub fn current_state() -> GstState {
    PLAYBIN.current_state()
}
#[instrument]
/// Current track position.
pub fn position() -> Option<ClockTime> {
    PLAYBIN.query_position::<ClockTime>()
}
#[instrument]
/// Current track duraiton.
pub fn duration() -> Option<ClockTime> {
    PLAYBIN.query_duration::<ClockTime>()
}
#[instrument]
/// Seek to a specified time in the current track.
pub async fn seek(time: ClockTime, flags: Option<SeekFlags>) -> Result<()> {
    let flags = if let Some(flags) = flags {
        flags
    } else {
        SeekFlags::FLUSH | SeekFlags::TRICKMODE_KEY_UNITS
    };

    PLAYBIN.seek_simple(flags, time)?;
    Ok(())
}
#[instrument]
/// Load the previous player state and seek to the last known position.
pub async fn resume(autoplay: bool) -> Result<()> {
    let mut state = QUEUE.get().unwrap().write().await;

    if let Some(last_position) = state.load_last_state().await {
        state.set_resume(true);

        let list = state.track_list();
        BROADCAST_CHANNELS
            .tx
            .broadcast(Notification::CurrentTrackList { list })
            .await?;

        if autoplay {
            state.set_target_status(GstState::Playing);
        } else {
            state.set_target_status(GstState::Paused);
        }

        if let Some(track) = state.current_track() {
            if let Some(url) = track.track_url {
                PLAYBIN.set_property("uri", url);

                ready().await?;
                pause().await?;

                seek(last_position, None).await?;

                return Ok(());
            } else {
                return Err(Error::Resume);
            }
        } else {
            return Err(Error::Resume);
        }
    }

    Ok(())
}
#[instrument]
/// Retreive controls for the player.
pub fn controls() -> Controls {
    CONTROLS.clone()
}
#[instrument]
/// Jump forward in the currently playing track +10 seconds.
pub async fn jump_forward() -> Result<()> {
    if let (Some(current_position), Some(duration)) = (
        PLAYBIN.query_position::<ClockTime>(),
        PLAYBIN.query_duration::<ClockTime>(),
    ) {
        let ten_seconds = ClockTime::from_seconds(10);
        let next_position = current_position + ten_seconds;

        if next_position < duration {
            seek(next_position, None).await?;
        } else {
            seek(duration, None).await?;
        }
    }

    Ok(())
}
#[instrument]
/// Jump forward in the currently playing track -10 seconds.
pub async fn jump_backward() -> Result<()> {
    if let Some(current_position) = PLAYBIN.query_position::<ClockTime>() {
        if current_position.seconds() < 10 {
            seek(ClockTime::default(), None).await?;
        } else {
            let ten_seconds = ClockTime::from_seconds(10);
            let seek_position = current_position - ten_seconds;

            seek(seek_position, None).await?;
        }
    }

    Ok(())
}
#[instrument]
/// Skip to the next, previous or specific track in the playlist.
pub async fn skip(direction: SkipDirection, num: Option<u32>) -> Result<()> {
    // Typical previous skip functionality where if,
    // the track is greater than 1 second into playing,
    // then it goes to the beginning. If triggered again
    // within a second after playing, it will skip to the previous track.
    if direction == SkipDirection::Backward {
        if let Some(current_position) = position() {
            if current_position.seconds() > 1 && num.is_none() {
                debug!("current track position >1s, seeking to start of track");

                let zero_clock = ClockTime::default();

                seek(zero_clock, None).await?;

                return Ok(());
            }
        }
    }

    let mut state = QUEUE.get().unwrap().write().await;
    let target_status = state.target_status();

    ready().await?;

    if let Some(next_track_to_play) = state.skip_track(num, direction.clone()).await {
        drop(state);

        if let Some(url) = &next_track_to_play.track_url {
            debug!("skipping {direction} to next track");

            PLAYBIN.set_property("uri", Some(url.clone()));

            set_player_state(target_status).await?;
        }
    }

    Ok(())
}
#[instrument]
/// Skip to a specific track in the current playlist
/// by its index in the list.
pub async fn skip_to(index: u32) -> Result<()> {
    let state = QUEUE.get().unwrap().read().await;

    if let Some(current_track) = state.current_track() {
        drop(state);

        let current_index = current_track.position;

        if index > current_index {
            debug!(
                "skipping forward from track {} to track {}",
                current_index, index
            );
            skip(SkipDirection::Forward, Some(index)).await?;
        } else {
            debug!(
                "skipping backward from track {} to track {}",
                current_index, index
            );
            skip(SkipDirection::Backward, Some(index)).await?;
        }
    }

    Ok(())
}
#[instrument]
/// Plays a single track.
pub async fn play_track(track_id: i32) -> Result<()> {
    ready().await?;

    if let Some(track_url) = QUEUE
        .get()
        .unwrap()
        .write()
        .await
        .play_track(track_id)
        .await
    {
        PLAYBIN.set_property("uri", Some(track_url.as_str()));

        play().await?;
    }

    Ok(())
}
#[instrument]
/// Plays a full album.
pub async fn play_album(album_id: String) -> Result<()> {
    ready().await?;

    if let Some(track_url) = QUEUE
        .get()
        .unwrap()
        .write()
        .await
        .play_album(album_id)
        .await
    {
        PLAYBIN.set_property("uri", Some(track_url));

        play().await?;
    }

    Ok(())
}
#[instrument]
/// Plays all tracks in a playlist.
pub async fn play_playlist(playlist_id: i64) -> Result<()> {
    ready().await?;

    if let Some(track_url) = QUEUE
        .get()
        .unwrap()
        .write()
        .await
        .play_playlist(playlist_id)
        .await
    {
        PLAYBIN.set_property("uri", Some(track_url.as_str()));

        play().await?;
    }

    Ok(())
}
#[instrument]
/// Play an item from Qobuz web uri
pub async fn play_uri(uri: String) -> Result<()> {
    match client::parse_url(uri.as_str()) {
        Ok(url) => match url {
            UrlType::Album { id } => {
                play_album(id).await?;
            }
            UrlType::Playlist { id } => {
                play_playlist(id).await?;
            }
            UrlType::Track { id } => {
                play_track(id).await?;
            }
        },
        Err(err) => {
            return Err(Error::FailedToPlay {
                message: format!("Failed to play item. {err}"),
            })
        }
    }

    Ok(())
}
#[instrument]
/// In response to the about-to-finish signal,
/// prepare the next track by downloading the stream url.
async fn prep_next_track() -> Result<()> {
    let mut state = QUEUE.get().unwrap().write().await;

    if let Some(next_track) = state.skip_track(None, SkipDirection::Forward).await {
        drop(state);

        debug!("received new track, adding to player");
        if let Some(next_playlist_track_url) = &next_track.track_url {
            PLAYBIN.set_property("uri", Some(next_playlist_track_url.clone()));
        }
    } else {
        debug!("no more tracks left");
    }

    Ok(())
}
#[instrument]
/// Get a notification channel receiver
pub fn notify_receiver() -> BroadcastReceiver {
    BROADCAST_CHANNELS.rx.clone()
}

#[instrument]
/// Returns the current track list loaded in the player.
pub async fn current_tracklist() -> TrackListValue {
    QUEUE.get().unwrap().read().await.track_list()
}

#[instrument]
/// Returns the current track loaded in the player.
pub async fn current_track() -> Option<Track> {
    QUEUE.get().unwrap().read().await.current_track()
}

#[instrument]
/// Returns true if the player is currently buffering data.
pub fn is_buffering() -> bool {
    IS_BUFFERING.load(Ordering::Relaxed)
}

#[instrument]
/// Search the service.
pub async fn search(query: &str) -> SearchResults {
    QUEUE
        .get()
        .unwrap()
        .read()
        .await
        .search_all(query)
        .await
        .unwrap_or_default()
}

#[instrument]
#[cached(size = 10, time = 600)]
/// Fetch the albums for a specific artist.
pub async fn artist_albums(artist_id: i32) -> Vec<Album> {
    if let Some(mut albums) = QUEUE
        .get()
        .unwrap()
        .read()
        .await
        .fetch_artist_albums(artist_id)
        .await
    {
        albums.sort_by_key(|a| a.release_year);

        albums
    } else {
        Vec::new()
    }
}

#[instrument]
#[cached(size = 10, time = 600)]
/// Fetch the tracks for a specific playlist.
pub async fn playlist_tracks(playlist_id: i64) -> Vec<Track> {
    if let Some(tracks) = QUEUE
        .get()
        .unwrap()
        .read()
        .await
        .fetch_playlist_tracks(playlist_id)
        .await
    {
        tracks
    } else {
        Vec::new()
    }
}

#[instrument]
#[cached(size = 1, time = 600)]
/// Fetch the current user's list of playlists.
pub async fn user_playlists() -> Vec<Playlist> {
    if let Some(playlists) = QUEUE
        .get()
        .unwrap()
        .read()
        .await
        .fetch_user_playlists()
        .await
    {
        playlists
    } else {
        Vec::new()
    }
}

/// Inserts the most recent position into the state at a set interval.
#[instrument]
pub async fn clock_loop() {
    debug!("starting clock loop");

    let mut interval = tokio::time::interval(Duration::from_millis(REFRESH_RESOLUTION));
    let mut last_position = ClockTime::default();

    loop {
        interval.tick().await;

        if current_state() == GstState::Playing {
            if let Some(position) = position() {
                if position.seconds() != last_position.seconds() {
                    last_position = position;

                    BROADCAST_CHANNELS
                        .tx
                        .broadcast(Notification::Position { clock: position })
                        .await
                        .expect("failed to send notification");
                }
            }
        }
    }
}

async fn quit() -> Result<()> {
    debug!("stopping player");

    if is_playing() {
        debug!("pausing player");
        pause().await?;
    }

    if is_paused() {
        debug!("readying player");
        ready().await?;
    }

    if is_ready() {
        debug!("stopping player");
        stop().await?;
    }

    BROADCAST_CHANNELS
        .tx
        .broadcast(Notification::Quit)
        .await
        .expect("error sending broadcast");

    Ok(())
}

/// Handles messages from GStreamer, receives player actions from external controls
/// receives the about-to-finish event and takes necessary action.
#[instrument]
pub async fn player_loop() -> Result<()> {
    let mut messages = PLAYBIN.bus().unwrap().stream();
    let mut about_to_finish = ABOUT_TO_FINISH.rx.stream();

    let action_rx = CONTROLS.action_receiver();
    let mut actions = action_rx.stream();
    let mut quitter = QUEUE.get().unwrap().read().await.quitter();

    let clock_handle = tokio::spawn(async { clock_loop().await });

    loop {
        select! {
            Ok(should_quit)= quitter.recv() => {
                if should_quit {
                    clock_handle.abort();
                    quit().await?;
                    break;
                }
            }
            Some(almost_done) = about_to_finish.next() => {
                if almost_done {
                    tokio::spawn(async { prep_next_track().await });
                }
            }
            Some(action) = actions.next() => {
                tokio::spawn(async { handle_action(action).await.expect("error handling action") });
            }
            Some(msg) = messages.next() => {
                if msg.type_() == MessageType::Buffering {
                    handle_message(msg).await?;
                } else {
                    tokio::spawn(async { handle_message(msg).await.expect("error handling message") });
                }
            }
        }
    }

    Ok(())
}

async fn handle_action(action: Action) -> Result<()> {
    match action {
        Action::JumpBackward => jump_backward().await?,
        Action::JumpForward => jump_forward().await?,
        Action::Next => {
            skip(SkipDirection::Forward, None).await?;
        }
        Action::Pause => pause().await?,
        Action::Play => play().await?,
        Action::PlayPause => play_pause().await?,
        Action::Previous => {
            skip(SkipDirection::Backward, None).await?;
        }
        Action::Stop => stop().await?,
        Action::PlayAlbum { album_id } => {
            play_album(album_id).await?;
        }
        Action::PlayTrack { track_id } => {
            play_track(track_id).await?;
        }
        Action::PlayUri { uri } => {
            play_uri(uri).await?;
        }
        Action::PlayPlaylist { playlist_id } => {
            play_playlist(playlist_id).await?;
        }
        Action::Quit => QUEUE.get().unwrap().read().await.quit(),
        Action::SkipTo { num } => {
            skip_to(num).await?;
        }
        Action::Search { query } => {
            search(&query).await;
        }
        Action::FetchArtistAlbums { artist_id: _ } => {}
        Action::FetchPlaylistTracks { playlist_id: _ } => {}
        Action::FetchUserPlaylists => {}
    }

    Ok(())
}

async fn handle_message(msg: Message) -> Result<()> {
    match msg.view() {
        MessageView::Eos(_) => {
            debug!("END OF STREAM");
            if QUIT_WHEN_DONE.load(Ordering::Relaxed) {
                QUEUE.get().unwrap().read().await.quit();
            } else {
                let mut state = QUEUE.get().unwrap().write().await;
                state.set_target_status(GstState::Paused);
                drop(state);

                skip(SkipDirection::Backward, Some(1)).await?;
            }
        }
        MessageView::AsyncDone(msg) => {
            debug!("ASYNC DONE");
            let position = if let Some(p) = msg.running_time() {
                p
            } else if let Some(p) = position() {
                p
            } else {
                ClockTime::default()
            };

            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::Position { clock: position })
                .await?;
        }
        MessageView::PropertyNotify(el) => {
            let (_, prop_name, value) = el.get();

            if let Some(v) = value {
                if prop_name == "caps" {
                    if let Ok(caps) = v.get::<&Caps>() {
                        if !caps.is_empty() {
                            if let Some(structure) = caps.structure(0) {
                                let rate: u32 = structure.get("rate").unwrap_or_default();
                                let format: &str = structure.get("format").unwrap_or_default();
                                let bits = if format.starts_with("S24") {
                                    24_u32
                                } else if format.starts_with("S16") {
                                    16_u32
                                } else {
                                    0
                                };

                                if rate == 0 || bits == 0 {
                                    return Ok(());
                                }

                                let previous_bits = BIT_DEPTH.swap(bits, Ordering::SeqCst);
                                let previous_rate = SAMPLING_RATE.swap(rate, Ordering::SeqCst);

                                if previous_rate != rate || previous_bits != bits {
                                    match BROADCAST_CHANNELS.tx.try_broadcast(
                                        Notification::AudioQuality {
                                            bitdepth: bits,
                                            sampling_rate: rate,
                                        },
                                    ) {
                                        Ok(_) => {}
                                        Err(err) => {
                                            debug!(?err);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        MessageView::StreamStart(_) => {
            debug!("stream start");

            let list = QUEUE.get().unwrap().read().await.track_list();
            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::CurrentTrackList { list })
                .await?;
        }
        MessageView::Buffering(buffering) => {
            if IS_LIVE.load(Ordering::Relaxed) {
                debug!("stream is live, ignore buffering");
                return Ok(());
            }
            let percent = buffering.percent();

            let target_status = QUEUE.get().unwrap().read().await.target_status();

            if percent < 100 && !is_paused() && !IS_BUFFERING.load(Ordering::Relaxed) {
                pause().await?;

                IS_BUFFERING.store(true, Ordering::Relaxed);
            } else if percent > 99 && IS_BUFFERING.load(Ordering::Relaxed) && is_paused() {
                set_player_state(target_status).await?;
                IS_BUFFERING.store(false, Ordering::Relaxed);
            }

            if percent.rem_euclid(10) == 0 {
                debug!("buffering {}%", percent);
                BROADCAST_CHANNELS
                    .tx
                    .broadcast(Notification::Buffering {
                        is_buffering: percent < 99,
                        target_status,
                        percent: percent as u32,
                    })
                    .await?;
            }
        }
        MessageView::StateChanged(state_changed) => {
            let current_state = state_changed
                .current()
                .to_value()
                .get::<GstState>()
                .unwrap();

            let mut state = QUEUE.get().unwrap().write().await;

            if state.status() != current_state && state.target_status() == current_state {
                debug!("player state changed {:?}", current_state);
                state.set_status(current_state);
                drop(state);

                BROADCAST_CHANNELS
                    .tx
                    .broadcast(Notification::Status {
                        status: current_state,
                    })
                    .await?;
            }
        }
        MessageView::ClockLost(_) => {
            debug!("clock lost, restarting playback");
            pause().await?;
            play().await?;
        }
        MessageView::Error(err) => {
            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::Error { error: err.into() })
                .await?;

            ready().await?;
            pause().await?;
            play().await?;

            debug!(
                "Error from {:?}: {} ({:?})",
                err.src().map(|s| s.path_string()),
                err.error(),
                err.debug()
            );
        }
        _ => (),
    }

    Ok(())
}

#[macro_export]
macro_rules! action {
    ($self:ident, $action:expr) => {
        if let Err(_) = $self.action_tx.send_async($action).await {
            error!("error sending action");
        }
    };
}

#[macro_export]
macro_rules! action_blocking {
    ($self:ident, $action:expr) => {
        if let Err(_) = $self.action_tx.send($action) {
            error!("error sending action");
        }
    };
}
