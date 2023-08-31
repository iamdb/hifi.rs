use crate::{
    player::{
        controls::{Action, Controls},
        error::Error,
        notification::{BroadcastReceiver, BroadcastSender, Notification},
    },
    qobuz::SearchResults,
    qobuz::{album::Album, track::Track},
    sql::db::Database,
    state::{
        app::{PlayerState, SafePlayerState, SkipDirection},
        ClockValue, StatusValue, TrackListValue,
    },
    REFRESH_RESOLUTION,
};
use flume::{Receiver, Sender};
use futures::prelude::*;
use gst::{
    prelude::*, ClockTime, Element, MessageView, SeekFlags, State as GstState, StateChangeSuccess,
    Structure,
};
use gstreamer as gst;
use hifirs_qobuz_api::client::{self, api::Client, UrlType};
use once_cell::sync::{Lazy, OnceCell};
use std::{
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{select, sync::RwLock};

#[macro_use]
pub mod controls;
pub mod error;
pub mod notification;

pub type Result<T, E = Error> = std::result::Result<T, E>;

static PLAYBIN: Lazy<Element> = Lazy::new(|| {
    gst::init().expect("error initializing gstreamer");

    let playbin = gst::ElementFactory::make("playbin3")
        .build()
        .expect("error building playbin element");

    playbin.set_property_from_str("flags", "audio+buffering");
    playbin.connect("element-setup", false, |value| {
        let element = &value[1].get::<gst::Element>().unwrap();

        if element.name().contains("urisourcebin") {
            element.set_property("parse-streams", true);
        }

        None
    });
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
    let (mut tx, rx) = async_broadcast::broadcast(10);
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
static STATE: OnceCell<SafePlayerState> = OnceCell::new();
static USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_4) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36"
];

#[instrument]
pub async fn init(client: Client, database: Database, quit_when_done: bool) -> Result<()> {
    let state = Arc::new(RwLock::new(PlayerState::new(client.clone(), database)));

    STATE.set(state).expect("error setting player state");
    QUIT_WHEN_DONE.store(quit_when_done, Ordering::Relaxed);

    Ok(())
}
/// Play the player.
#[instrument]
pub async fn play(wait: bool) -> Result<()> {
    set_player_state(gst::State::Playing, wait).await?;
    Ok(())
}
/// Pause the player.
#[instrument]
pub async fn pause(wait: bool) -> Result<()> {
    set_player_state(gst::State::Paused, wait).await?;
    Ok(())
}
/// Ready the player.
#[instrument]
pub async fn ready(wait: bool) -> Result<()> {
    set_player_state(gst::State::Ready, wait).await?;
    Ok(())
}
/// Stop the player.
#[instrument]
pub async fn stop(wait: bool) -> Result<()> {
    set_player_state(gst::State::Null, wait).await?;
    Ok(())
}
/// Sets the player to a specific state.
#[instrument]
pub async fn set_player_state(state: gst::State, wait: bool) -> Result<()> {
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

    if wait {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        while current_state() != state.into() {
            debug!(
                "waiting for player to change to {}",
                current_state().as_str()
            );
            interval.tick().await;
        }
    }

    Ok(())
}
/// Toggle play and pause.
#[instrument]
pub async fn play_pause() -> Result<()> {
    let mut state = STATE.get().unwrap().write().await;

    if is_playing() {
        state.set_target_status(GstState::Paused);
        pause(false).await?;
    } else if is_paused() || is_ready() {
        state.set_target_status(GstState::Playing);
        play(false).await?;
    }

    Ok(())
}
/// Is the player paused?
#[instrument]
pub fn is_paused() -> bool {
    PLAYBIN.current_state() == gst::State::Paused
}
/// Is the player playing?
#[instrument]
pub fn is_playing() -> bool {
    PLAYBIN.current_state() == gst::State::Playing
}
/// Is the player ready?
#[instrument]
pub fn is_ready() -> bool {
    PLAYBIN.current_state() == gst::State::Ready
}
/// Current player state
#[instrument]
pub fn current_state() -> StatusValue {
    PLAYBIN.current_state().into()
}
/// Current track position.
#[instrument]
pub fn position() -> Option<ClockValue> {
    PLAYBIN
        .query_position::<ClockTime>()
        .map(|position| position.into())
}
/// Current track duraiton.
#[instrument]
pub fn duration() -> Option<ClockValue> {
    PLAYBIN
        .query_duration::<ClockTime>()
        .map(|duration| duration.into())
}
/// Seek to a specified time in the current track.
#[instrument]
pub async fn seek(time: ClockValue, flags: Option<SeekFlags>) -> Result<()> {
    let flags = if let Some(flags) = flags {
        flags
    } else {
        SeekFlags::FLUSH | SeekFlags::TRICKMODE_KEY_UNITS
    };

    PLAYBIN.seek_simple(flags, time.inner_clocktime())?;
    Ok(())
}
/// Load the previous player state and seek to the last known position.
#[instrument]
pub async fn resume(autoplay: bool) -> Result<()> {
    let mut state = STATE.get().unwrap().write().await;

    if state.load_last_state().await {
        state.set_resume(true);

        if autoplay {
            state.set_target_status(GstState::Playing);
        } else {
            state.set_target_status(GstState::Paused);
        }

        BROADCAST_CHANNELS
            .tx
            .broadcast(Notification::CurrentTrackList {
                list: state.track_list(),
            })
            .await?;

        if let Some(track) = state.current_track() {
            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::CurrentTrack {
                    track: track.clone(),
                })
                .await?;

            if let Some(url) = track.track_url {
                PLAYBIN.set_property("uri", url);

                ready(true).await?;
                pause(true).await?;

                let position = state.position();

                seek(position.clone(), None).await?;

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
/// Retreive controls for the player.
#[instrument]
pub fn controls() -> Controls {
    CONTROLS.clone()
}
/// Jump forward in the currently playing track +10 seconds.
#[instrument]
pub async fn jump_forward() -> Result<()> {
    if let (Some(current_position), Some(duration)) = (
        PLAYBIN.query_position::<ClockTime>(),
        PLAYBIN.query_duration::<ClockTime>(),
    ) {
        let ten_seconds = ClockTime::from_seconds(10);
        let next_position = current_position + ten_seconds;

        if next_position < duration {
            seek(next_position.into(), None).await?;
        } else {
            seek(duration.into(), None).await?;
        }
    }

    Ok(())
}
/// Jump forward in the currently playing track -10 seconds.
#[instrument]
pub async fn jump_backward() -> Result<()> {
    if let Some(current_position) = PLAYBIN.query_position::<ClockTime>() {
        if current_position.seconds() < 10 {
            seek(ClockTime::default().into(), None).await?;
        } else {
            let ten_seconds = ClockTime::from_seconds(10);
            let seek_position = current_position - ten_seconds;

            seek(seek_position.into(), None).await?;
        }
    }

    Ok(())
}
/// Skip to the next, previous or specific track in the playlist.
#[instrument]
pub async fn skip(direction: SkipDirection, num: Option<usize>) -> Result<()> {
    // Typical previous skip functionality where if,
    // the track is greater than 1 second into playing,
    // then it goes to the beginning. If triggered again
    // within a second after playing, it will skip to the previous track.
    if direction == SkipDirection::Backward {
        if let Some(current_position) = position() {
            if current_position.inner_clocktime().seconds() > 1 && num.is_none() {
                debug!("current track position >1s, seeking to start of track");

                let zero_clock: ClockValue = ClockTime::default().into();

                seek(zero_clock.clone(), None).await?;

                return Ok(());
            }
        }
    }

    let mut state = STATE.get().unwrap().write().await;
    let target_status = state.target_status();
    if let Some(next_track_to_play) = state.skip_track(num, direction.clone()).await {
        drop(state);

        if let Some(url) = &next_track_to_play.track_url {
            debug!("skipping {direction} to next track");

            ready(false).await?;
            PLAYBIN.set_property("uri", Some(url.clone()));
            set_player_state(target_status.into(), false).await?;

            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::CurrentTrackList {
                    list: STATE.get().unwrap().read().await.track_list(),
                })
                .await?;
            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::CurrentTrack {
                    track: next_track_to_play,
                })
                .await?;
        }
    }

    Ok(())
}
/// Skip to a specific track in the current playlist
/// by its index in the list.
#[instrument]
pub async fn skip_to(index: usize) -> Result<()> {
    let state = STATE.get().unwrap().read().await;

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
/// Skip to a specific track in the current playlist, by the
/// track id.
#[instrument]
pub async fn skip_to_by_id(track_id: usize) -> Result<()> {
    if let Some(track_number) = STATE.get().unwrap().read().await.track_index(track_id) {
        skip_to(track_number).await?;
    }

    Ok(())
}
/// Plays a single track.
#[instrument]
pub async fn play_track(track_id: i32) -> Result<()> {
    if !is_ready() {
        ready(true).await?;
    }

    if let (Some(track_list_track), Some(tracklist)) = STATE
        .get()
        .unwrap()
        .write()
        .await
        .play_track(track_id)
        .await
    {
        if let Some(track_url) = &track_list_track.track_url {
            PLAYBIN.set_property("uri", Some(track_url.as_str()));

            if !is_playing() {
                play(false).await?;
            }

            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::CurrentTrackList { list: tracklist })
                .await?;

            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::CurrentTrack {
                    track: track_list_track.clone(),
                })
                .await?;
        }
    }

    Ok(())
}
/// Plays a full album.
#[instrument]
pub async fn play_album(album_id: String) -> Result<()> {
    if !is_ready() {
        ready(true).await?;
    }

    if let (Some(track), Some(tracklist)) = STATE
        .get()
        .unwrap()
        .write()
        .await
        .play_album(album_id)
        .await
    {
        if let Some(track_url) = &track.track_url {
            PLAYBIN.set_property("uri", Some(track_url));

            if !is_playing() {
                play(false).await?;
            }

            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::CurrentTrackList {
                    list: tracklist.clone(),
                })
                .await?;

            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::CurrentTrack {
                    track: track.clone(),
                })
                .await?;
        }
    }

    Ok(())
}
/// Play an item from Qobuz web uri
#[instrument]
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
/// Plays all tracks in a playlist.
#[instrument]
pub async fn play_playlist(playlist_id: i64) -> Result<()> {
    if !is_ready() {
        ready(true).await?;
    }

    if let (Some(first_track), Some(tracklist)) = STATE
        .get()
        .unwrap()
        .write()
        .await
        .play_playlist(playlist_id)
        .await
    {
        if let Some(t) = &first_track.track_url {
            PLAYBIN.set_property("uri", Some(t.as_str()));

            if !is_playing() {
                play(false).await?;
            }

            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::CurrentTrackList {
                    list: tracklist.clone(),
                })
                .await?;

            BROADCAST_CHANNELS
                .tx
                .broadcast(Notification::CurrentTrack {
                    track: first_track.clone(),
                })
                .await?;
        }
    }

    Ok(())
}
/// In response to the about-to-finish signal,
/// prepare the next track by downloading the stream url.
#[instrument]
async fn prep_next_track() -> Result<()> {
    let mut state = STATE.get().unwrap().write().await;

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
/// Get a notification channel receiver
#[instrument]
pub fn notify_receiver() -> BroadcastReceiver {
    BROADCAST_CHANNELS.rx.clone()
}

#[instrument]
pub async fn current_tracklist() -> TrackListValue {
    STATE.get().unwrap().read().await.track_list()
}

#[instrument]
pub async fn current_track() -> Option<Track> {
    STATE.get().unwrap().read().await.current_track()
}

#[instrument]
pub fn is_buffering() -> bool {
    IS_BUFFERING.load(Ordering::Relaxed)
}

#[instrument]
pub async fn search(query: &str) -> SearchResults {
    STATE
        .get()
        .unwrap()
        .read()
        .await
        .search_all(query)
        .await
        .unwrap_or_default()
        .into()
}

#[instrument]
pub async fn artist_albums(artist_id: i32) -> Vec<Album> {
    STATE
        .get()
        .unwrap()
        .read()
        .await
        .fetch_artist_albums(artist_id)
        .await
}

#[instrument]
pub async fn playlist_tracks(artist_id: i32) -> Vec<Album> {
    STATE
        .get()
        .unwrap()
        .read()
        .await
        .fetch_artist_albums(artist_id)
        .await
}

/// Inserts the most recent position into the state at a set interval.
#[instrument]
pub async fn clock_loop() {
    debug!("starting clock loop");

    let mut interval = tokio::time::interval(Duration::from_millis(REFRESH_RESOLUTION));
    let mut last_position = ClockValue::default();

    loop {
        interval.tick().await;

        if current_state() == GstState::Playing.into() {
            if let Some(position) = position() {
                if position.inner_clocktime().seconds() != last_position.inner_clocktime().seconds()
                {
                    last_position = position.clone();

                    let mut state = STATE.get().unwrap().write().await;
                    state.set_position(position.clone());
                    drop(state);

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

/// Handles messages from GStreamer, receives player actions from external controls
/// receives the about-to-finish event and takes necessary action.
#[instrument]
pub async fn player_loop() -> Result<()> {
    let mut messages = PLAYBIN.bus().unwrap().stream();
    let mut about_to_finish = ABOUT_TO_FINISH.rx.stream();

    let action_rx = CONTROLS.action_receiver();
    let mut actions = action_rx.stream();
    let mut quitter = STATE.get().unwrap().read().await.quitter();

    let clock_handle = tokio::spawn(async { clock_loop().await });

    loop {
        select! {
            quit = quitter.recv() => {
                match quit {
                    Ok(quit) => {
                        if quit {
                            debug!("quitting player loop, exiting application");

                            clock_handle.abort();

                            if is_playing() {
                                debug!("pausing player");
                                pause(true).await?;
                            }

                            if is_paused() {
                                debug!("readying player");
                                ready(true).await?;
                            }


                            if is_ready() {
                                debug!("stopping player");
                                stop(true).await?;
                            }

                            std::process::exit(1);
                        }
                    },
                    Err(_) => {
                        debug!("quitting player loop, with error");
                        break;
                    },
                }
            }
            Some(almost_done) = about_to_finish.next() => {
                if almost_done {
                    tokio::spawn(async { prep_next_track().await });
                }
            }
            Some(action) = actions.next() => {
                match action {
                    Action::JumpBackward => jump_backward().await?,
                    Action::JumpForward => jump_forward().await?,
                    Action::Next => {
                        skip(SkipDirection::Forward,None).await?;
                    },
                    Action::Pause => pause(false).await?,
                    Action::Play => play(false).await?,
                    Action::PlayPause => play_pause().await?,
                    Action::Previous => {
                        skip(SkipDirection::Backward,None).await?;
                    },
                    Action::Stop => stop(false).await?,
                    Action::PlayAlbum { album_id } => {
                        play_album(album_id).await?;
                    },
                    Action::PlayTrack { track_id } => {
                        play_track(track_id).await?;
                    },
                    Action::PlayUri { uri } => {
                        play_uri(uri).await?;
                    },
                    Action::PlayPlaylist { playlist_id } => {
                        play_playlist(playlist_id).await?;
                    },
                    Action::Quit => STATE.get().unwrap().read().await.quit(),
                    Action::SkipTo { num } => {
                        skip_to(num).await?;
                    },
                    Action::SkipToById { track_id } => {
                        skip_to_by_id(track_id).await?;
                    },
                    Action::Search { query } => {
                       search(&query).await;
                    }
                    Action::FetchArtistAlbums {artist_id: _} => {}
                    Action::FetchPlaylistTracks{playlist_id: _} => {}
                }
            }
            Some(msg) = messages.next() => {
                match msg.view() {
                    MessageView::Eos(_) => {
                        debug!("END OF STREAM");
                        if QUIT_WHEN_DONE.load(Ordering::Relaxed) {
                            STATE.get().unwrap().read().await.quit();
                        } else {
                            let mut state = STATE.get().unwrap().write().await;
                            state.set_target_status(GstState::Paused);
                            drop(state);

                            skip(SkipDirection::Backward, Some(1)).await?;
                        }
                    },
                    MessageView::AsyncDone(msg) => {
                        debug!("ASYNC DONE");
                        let position = if let Some(p)= msg.running_time() {
                            p.into()
                        } else if let Some(p) = position() {
                            p
                        } else {
                            ClockTime::default().into()
                        };

                        BROADCAST_CHANNELS.tx.broadcast(Notification::Position { clock: position }).await?;
                    }
                    MessageView::StreamStart(_) => {
                        debug!("stream start");
                        if let Some(current_track) = STATE.get().unwrap().read().await.current_track() {
                            BROADCAST_CHANNELS.tx
                                .broadcast(Notification::CurrentTrack { track: current_track })
                                .await?;
                        }

                        let list = STATE.get().unwrap().read().await.track_list();
                        BROADCAST_CHANNELS.tx.broadcast(Notification::CurrentTrackList{ list }).await?;

                        if let Some(duration) = duration() {
                            debug!("setting track duration");
                            BROADCAST_CHANNELS.tx.broadcast(Notification::Duration { clock: duration }).await?;
                        }
                    }
                    MessageView::Buffering(buffering) => {
                        if IS_LIVE.load(Ordering::Relaxed) {
                            debug!("stream is live, ignore buffering");
                            continue;
                        }
                        let percent = buffering.percent();

                        let target_status = STATE.get().unwrap().read().await.target_status();

                        if percent < 100 && !is_paused() && !IS_BUFFERING.load(Ordering::Relaxed) {
                            pause(false).await?;

                            IS_BUFFERING.store(true, Ordering::Relaxed);
                        } else if percent > 99 && IS_BUFFERING.load(Ordering::Relaxed) && is_paused() {
                            set_player_state(target_status.clone().into(), false).await?;
                            IS_BUFFERING.store(false, Ordering::Relaxed);
                        }

                        if percent.rem_euclid(5) == 0 {
                            debug!("buffering {}%", percent);
                            BROADCAST_CHANNELS.tx.broadcast(Notification::Buffering { is_buffering: percent < 99, target_status, percent }).await?;
                        }
                    }
                    MessageView::StateChanged(state_changed) => {
                        let current_state = state_changed
                            .current()
                            .to_value()
                            .get::<GstState>()
                            .unwrap();

                        let mut state = STATE.get().unwrap().write().await;

                        if state.status() != current_state.into() && state.target_status() == current_state.into() {
                            debug!("player state changed {:?}", current_state);
                            state.set_status(current_state.into());
                            drop(state);

                            BROADCAST_CHANNELS.tx.broadcast(Notification::Status { status: current_state.into() }).await?;
                        }
                    }
                    MessageView::ClockLost(_) => {
                        debug!("clock lost, restarting playback");
                        pause(true).await?;
                        play(true).await?;
                    }
                    MessageView::Error(err) => {
                        BROADCAST_CHANNELS.tx.broadcast(Notification::Error { error: err.into() }).await?;

                        ready(true).await?;
                        pause(true).await?;
                        play(true).await?;

                        debug!(
                            "Error from {:?}: {} ({:?})",
                            err.src().map(|s| s.path_string()),
                            err.error(),
                            err.debug()
                        );
                    }
                    _ => (),
                }

            }
        }
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
