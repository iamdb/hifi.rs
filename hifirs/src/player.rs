use crate::{
    action,
    mpris::{self, MprisPlayer, MprisTrackList},
    sql::db::Database,
    state::{
        app::{PlayerKey, StateKey},
        ClockValue, FloatValue, StatusValue, TrackListValue,
    },
    REFRESH_RESOLUTION,
};
use flume::{Receiver, Sender};
use futures::prelude::*;
use gst::{bus::BusStream, glib, ClockTime, Element, MessageView, SeekFlags, State as GstState};
use gstreamer::{self as gst, prelude::*};
use qobuz_client::client::{
    self,
    album::Album,
    api::Client,
    playlist::Playlist,
    track::{Track, TrackListTrack},
    AudioQuality, TrackURL,
};
use snafu::prelude::*;
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::{select, sync::Mutex};
use zbus::Connection;

#[derive(Snafu, Debug)]
pub enum Error {
    #[snafu(display("Failed to retrieve a track url."))]
    TrackURL,
    #[snafu(display("Failed to seek."))]
    Seek,
    #[snafu(display("Sorry, could not resume previous session."))]
    Session,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum Action {
    Play,
    Pause,
    PlayPause,
    Next,
    Previous,
    Stop,
    Quit,
    SkipTo { num: usize },
    SkipToById { track_id: usize },
    JumpForward,
    JumpBackward,
    PlayAlbum { album: Box<Album> },
    PlayTrack { track: Box<Track> },
    PlayUri { uri: String },
    PlayPlaylist { playlist: Box<Playlist> },
}

/// A player handles playing media to a device.
#[derive(Debug, Clone)]
pub struct Player {
    /// Used to broadcast the player state out to other components.
    playbin: Element,
    /// List of tracks that will play.
    tracklist: Arc<Mutex<TrackListValue>>,
    /// List of tracks that have played.
    previous_tracklist: Arc<Mutex<TrackListValue>>,
    /// The app state to save player inforamtion into.
    db: Database,
    /// Qobuz client
    client: Client,
    controls: Controls,
    connection: Connection,
    is_buffering: bool,
    resume: bool,
}

pub async fn new(db: Database, client: Client, resume: bool) -> Player {
    gst::init().expect("Couldn't initialize Gstreamer");
    let playbin = gst::ElementFactory::make("playbin", None).expect("failed to create gst element");
    let controls = Controls::new(db.clone());
    let tracklist = Arc::new(Mutex::new(TrackListValue::new()));
    let previous_tracklist = Arc::new(Mutex::new(TrackListValue::new()));

    let connection = mpris::init(controls.clone()).await;

    let (about_to_finish_tx, about_to_finish_rx) = flume::bounded::<bool>(1);
    let (next_track_tx, next_track_rx) = flume::bounded::<String>(1);

    // Connects to the `about-to-finish` signal so the player
    // can setup the next track to play. Enables gapless playback.
    playbin.connect("about-to-finish", false, move |values| {
        debug!("about to finish");
        about_to_finish_tx
            .send(true)
            .expect("failed to send about to finish message");

        if let Ok(next_track_url) = next_track_rx.recv_timeout(Duration::from_secs(15)) {
            let playbin = values[0]
                .get::<glib::Object>()
                .expect("playbin \"about-to-finish\" signal values[0]");

            playbin.set_property("uri", Some(next_track_url));
        }

        None
    });

    let mut player = Player {
        db,
        connection,
        client,
        playbin,
        tracklist,
        previous_tracklist,
        controls,
        is_buffering: false,
        resume,
    };

    if resume {
        player.resume().await.expect("failed to resume");
    }

    let p = player.clone();
    tokio::spawn(async move {
        p.clock_loop().await;
    });

    let mut p = player.clone();
    tokio::spawn(async move {
        p.player_loop(about_to_finish_rx, next_track_tx).await;
    });

    player
}

impl Player {
    /// Set the active playlist.
    pub fn set_playlist(&mut self, playlist: TrackListValue) {
        self.tracklist = Arc::new(Mutex::new(playlist));
    }
    /// Set the previous playlist.
    pub fn set_prev_playlist(&mut self, playlist: TrackListValue) {
        self.previous_tracklist = Arc::new(Mutex::new(playlist));
    }
    /// Play the player.
    pub fn play(&self) {
        self.playbin
            .set_state(gst::State::Playing)
            .expect("Unable to set the pipeline to the `Playing` state");
    }
    /// Pause the player.
    pub fn pause(&self) {
        self.playbin
            .set_state(gst::State::Paused)
            .expect("Unable to set the pipeline to the `Paused` state");
    }
    /// Toggle play and pause.
    pub fn play_pause(&self) {
        if self.is_playing() {
            self.pause();
        } else if self.is_paused() {
            self.play()
        }
    }
    /// Ready the player.
    pub fn ready(&self) {
        self.playbin
            .set_state(gst::State::Ready)
            .expect("Unable to set the pipeline to the `Ready` state");
    }
    /// Stop the player.
    pub fn stop(&self) {
        self.playbin
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");
    }
    /// Set the uri of the track to play.
    pub fn set_uri(&self, track_url: TrackURL) {
        self.playbin
            .set_property("uri", Some(track_url.url.as_str()));
    }
    /// Is the player paused?
    pub fn is_paused(&self) -> bool {
        self.playbin.current_state() == gst::State::Paused
    }
    /// Is the player playing?
    pub fn is_playing(&self) -> bool {
        self.playbin.current_state() == gst::State::Playing
    }
    /// Current player state
    pub fn current_state(&self) -> StatusValue {
        self.playbin.current_state().into()
    }
    pub fn position(&self) -> Option<ClockValue> {
        self.playbin
            .query_position::<ClockTime>()
            .map(|position| position.into())
    }
    pub fn duration(&self) -> Option<ClockValue> {
        self.playbin
            .query_duration::<ClockTime>()
            .map(|duration| duration.into())
    }
    /// Seek to a specified time in the current track.
    pub async fn seek(&self, time: ClockValue, flags: Option<SeekFlags>) -> Result<()> {
        let flags = if let Some(flags) = flags {
            flags
        } else {
            SeekFlags::FLUSH | SeekFlags::KEY_UNIT
        };

        match self.playbin.seek_simple(flags, time.inner_clocktime()) {
            Ok(_) => {
                self.db
                    .insert::<String, ClockValue>(
                        StateKey::Player(PlayerKey::Position),
                        time.clone(),
                    )
                    .await;

                self.dbus_seeked_signal(time).await;

                Ok(())
            }
            Err(error) => {
                error!("{}", error.message);
                Err(Error::Seek)
            }
        }
    }
    pub async fn resume(&mut self) -> Result<()> {
        if let (Some(playlist), Some(next_up)) = (
            self.db
                .get::<String, TrackListValue>(StateKey::Player(PlayerKey::Playlist))
                .await,
            self.db
                .get::<String, TrackListTrack>(StateKey::Player(PlayerKey::NextUp))
                .await,
        ) {
            let next_track = self.attach_track_url(next_up).await?;

            if let Some(track_url) = next_track.track_url {
                self.set_playlist(playlist);
                self.set_uri(track_url);

                if let Some(prev_playlist) = self
                    .db
                    .get::<String, TrackListValue>(StateKey::Player(PlayerKey::PreviousPlaylist))
                    .await
                {
                    self.set_prev_playlist(prev_playlist);

                    self.pause();
                }
            }
            Ok(())
        } else {
            debug!("nothing to resume");
            self.resume = false;
            Ok(())
        }
    }
    /// Retreive controls for the player.
    pub fn controls(&self) -> Controls {
        self.controls.clone()
    }
    pub async fn clear(&mut self) {
        self.tracklist.lock().await.clear();
        self.previous_tracklist.lock().await.clear();
        self.db.clear_state().await;
    }
    /// Jump forward in the currently playing track +10 seconds.
    pub async fn jump_forward(&self) {
        if let (Some(current_position), Some(duration)) = (
            self.playbin.query_position::<ClockTime>(),
            self.playbin.query_duration::<ClockTime>(),
        ) {
            let ten_seconds = ClockTime::from_seconds(10);
            let next_position = current_position + ten_seconds;

            if next_position < duration {
                match self.seek(next_position.into(), None).await {
                    Ok(_) => (),
                    Err(error) => {
                        error!("{:?}", error);
                    }
                }
            } else {
                match self.seek(duration.into(), None).await {
                    Ok(_) => (),
                    Err(error) => {
                        error!("{:?}", error);
                    }
                }
            }
        }
    }
    /// Jump forward in the currently playing track -10 seconds.
    pub async fn jump_backward(&self) {
        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            if current_position.seconds() < 10 {
                self.seek(ClockTime::default().into(), None)
                    .await
                    .expect("failed to jump backward");
            } else {
                let ten_seconds = ClockTime::from_seconds(10);
                let seek_position = current_position - ten_seconds;

                self.seek(seek_position.into(), None)
                    .await
                    .expect("failed to jump backward");
            }
        }
    }
    /// Skip forward to the next track in the playlist.
    pub async fn skip_forward(&self, num: Option<usize>) -> Result<()> {
        let mut playlist = self.tracklist.lock().await;
        let mut prev_playlist = self.previous_tracklist.lock().await;

        // Do nothing if it's the first track,
        // which we know by the playlist being
        // empty.
        if playlist.len() == 0 {
            return Ok(());
        }

        if let Some(previous_track) = self
            .db
            .get::<String, TrackListTrack>(StateKey::Player(PlayerKey::NextUp))
            .await
        {
            prev_playlist.push_back(previous_track);
        }

        if let Some(number) = num {
            // Grab all of the tracks, up to the next one to play.
            prev_playlist.append(
                playlist
                    .drain(..number)
                    .collect::<VecDeque<TrackListTrack>>(),
            );
        }

        if let Some(mut next_track_to_play) = playlist.pop_front() {
            debug!("fetching url for next track");

            next_track_to_play = self.attach_track_url(next_track_to_play).await?;

            if let Some(track_url) = next_track_to_play.clone().track_url {
                debug!("skipping forward to next track");
                self.ready();

                self.db
                    .insert::<String, TrackListTrack>(
                        StateKey::Player(PlayerKey::NextUp),
                        next_track_to_play.clone(),
                    )
                    .await;

                self.db
                    .insert::<String, TrackListValue>(
                        StateKey::Player(PlayerKey::Playlist),
                        playlist.clone(),
                    )
                    .await;

                self.db
                    .insert::<String, TrackListValue>(
                        StateKey::Player(PlayerKey::PreviousPlaylist),
                        prev_playlist.clone(),
                    )
                    .await;

                self.playbin.set_property("uri", Some(track_url.url));
                self.play();

                self.dbus_seeked_signal(ClockValue::default()).await;
                self.dbus_metadata_changed().await;
            }
        }
        Ok(())
    }
    /// Skip backwards by playing the first track in previous track playlist.
    pub async fn skip_backward(&self, num: Option<usize>) -> Result<()> {
        // If the track is greater than 1 second into playing,
        // then we just want to go back to the beginning.
        // If triggered again within a second after playing,
        // it will skip to the previous track.
        if let Some(current_position) = self.playbin.query_position::<ClockTime>() {
            let one_second = ClockTime::from_seconds(1);

            if current_position > one_second && num.is_none() {
                debug!("current track position >1s, seeking to start of track");
                self.seek(ClockTime::default().into(), None)
                    .await
                    .expect("failed to seek");

                self.dbus_seeked_signal(ClockValue::default()).await;
                self.dbus_metadata_changed().await;

                return Ok(());
            }
        }

        let mut playlist = self.tracklist.lock().await;
        let mut prev_playlist = self.previous_tracklist.lock().await;

        // Do nothing if it's the first track,
        // which we know by the previous list being
        // empty.
        if prev_playlist.len() == 0 {
            return Ok(());
        }

        if let Some(previously_played_track) = self
            .db
            .get::<String, TrackListTrack>(StateKey::Player(PlayerKey::NextUp))
            .await
        {
            playlist.push_front(previously_played_track);
        }

        if let Some(number) = num {
            // Grab all of the tracks, up to the next one to play,
            // inlcuding the currently playing track from above.
            let diff = number + 1 - playlist.len();
            let skipped_tracks = prev_playlist
                .drain(diff + 1..)
                .rev()
                .collect::<VecDeque<TrackListTrack>>();

            for track in skipped_tracks {
                playlist.push_front(track);
            }
        }

        if let Some(mut next_track_to_play) = prev_playlist.pop_back() {
            next_track_to_play = self.attach_track_url(next_track_to_play).await?;

            if let Some(track_url) = next_track_to_play.clone().track_url {
                debug!("skipping backward to previous track");
                self.ready();

                self.db
                    .insert::<String, TrackListTrack>(
                        StateKey::Player(PlayerKey::NextUp),
                        next_track_to_play.clone(),
                    )
                    .await;

                self.db
                    .insert::<String, TrackListValue>(
                        StateKey::Player(PlayerKey::Playlist),
                        playlist.clone(),
                    )
                    .await;

                self.db
                    .insert::<String, TrackListValue>(
                        StateKey::Player(PlayerKey::PreviousPlaylist),
                        prev_playlist.clone(),
                    )
                    .await;

                self.dbus_metadata_changed().await;
                self.dbus_seeked_signal(ClockValue::default()).await;

                self.playbin.set_property("uri", Some(track_url.url));
                self.play();
            }
        }

        Ok(())
    }
    /// Skip to a specific track in the current playlist
    /// by its index in the list.
    pub async fn skip_to(&self, index: usize) -> Result<()> {
        if index < self.tracklist.lock().await.len() {
            debug!("skipping forward to track number {}", index);
            self.skip_forward(Some(index)).await?;
        } else {
            debug!("skipping backward to track number {}", index);
            self.skip_backward(Some(index)).await?;
        }

        Ok(())
    }
    /// Skip to a specific track in the current playlist, by the
    /// track id.
    pub async fn skip_to_by_id(&mut self, track_id: usize) -> Result<()> {
        let playlist = self.tracklist.lock().await;
        let prev_playlist = self.previous_tracklist.lock().await;

        if let Some(track_number) = playlist.track_index(track_id) {
            self.skip_to(track_number).await?;
        } else if let Some(track_number) = prev_playlist.track_index(track_id) {
            self.skip_to(track_number).await?;
        }

        Ok(())
    }
    /// Plays a single track.
    pub async fn play_track(&mut self, track: Track, quality: Option<AudioQuality>) {
        if self.is_playing() {
            self.stop();
        }

        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        let playlist_track =
            TrackListTrack::new(track, Some(0), Some(1), Some(quality.clone()), None);
        self.tracklist.lock().await.push_back(playlist_track);

        self.start(quality).await;
    }
    /// Plays a full album.
    pub async fn play_album(&mut self, mut album: Album, quality: Option<AudioQuality>) {
        if self.is_playing() || self.is_paused() {
            self.stop();
        }

        self.clear().await;

        if album.tracks.is_none() {
            album.attach_tracks(self.client.clone()).await;
        }

        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        if let Some(tracklist) = album.to_tracklist(quality.clone()) {
            debug!("creating tracklist");
            debug!("creating playlist");
            let mut current_tracklist = self.tracklist.lock().await;

            for playlist_track in tracklist {
                current_tracklist.push_back(playlist_track);
            }

            current_tracklist.set_album(album.clone());

            if let Some(tracks) = album.tracks {
                let tracks = tracks
                    .items
                    .iter()
                    .map(|t| t.id.to_string())
                    .collect::<Vec<String>>();

                let current = tracks.first().cloned().unwrap();

                self.dbus_track_list_replaced_signal(tracks, current).await;
            }

            self.dbus_metadata_changed().await;
        }

        self.start(quality).await;
    }
    /// Play an item from Qobuz web uri
    pub async fn play_uri(&mut self, uri: String, quality: Option<AudioQuality>) {
        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        if let Some(url) = client::parse_url(uri.as_str()) {
            match url {
                client::UrlType::Album { id } => {
                    if let Ok(album) = self.client.album(id).await {
                        self.play_album(album, Some(quality)).await;
                    }
                }
                client::UrlType::Playlist { id } => {
                    if let Ok(playlist) = self.client.playlist(id).await {
                        self.play_playlist(playlist, Some(quality)).await;
                    }
                }
            }
        }
    }
    pub async fn play_playlist(&mut self, mut playlist: Playlist, quality: Option<AudioQuality>) {
        if self.is_playing() || self.is_paused() {
            self.stop();
        }

        self.clear().await;

        let quality = if let Some(quality) = quality {
            quality
        } else {
            self.client.quality()
        };

        if let Some(tracklist) = playlist.to_tracklist(Some(quality.clone())) {
            debug!("creating playlist");
            let mut current_tracklist = self.tracklist.lock().await;
            for playlist_track in tracklist {
                current_tracklist.push_back(playlist_track);
            }

            current_tracklist.set_playlist(playlist.clone());

            if let Some(tracks) = playlist.tracks {
                let tracks = tracks
                    .items
                    .iter()
                    .map(|t| t.id.to_string())
                    .collect::<Vec<String>>();

                let current = tracks.first().cloned().unwrap();

                self.dbus_track_list_replaced_signal(tracks, current).await;
            }

            self.dbus_metadata_changed().await;
        }

        self.start(quality).await;
    }
    /// Starts the player.
    async fn start(&mut self, quality: AudioQuality) {
        let mut tracklist = self.tracklist.lock().await;

        let mut next_track = match tracklist.pop_front() {
            Some(it) => it,
            _ => return,
        };
        let playbin = &self.playbin;

        if let Ok(track_url) = self
            .client
            .track_url(next_track.track.id, Some(quality.clone()), None)
            .await
        {
            playbin.set_property("uri", Some(track_url.url.as_str()));
            next_track.set_track_url(track_url);

            self.db
                .insert::<String, TrackListTrack>(
                    StateKey::Player(PlayerKey::NextUp),
                    next_track.clone(),
                )
                .await;

            self.db
                .insert::<String, TrackListValue>(
                    StateKey::Player(PlayerKey::Playlist),
                    tracklist.clone(),
                )
                .await;

            self.play();
        }
    }
    /// Handles messages from the player and takes necessary action.
    async fn player_loop(
        &mut self,
        about_to_finish_rx: Receiver<bool>,
        next_track_tx: Sender<String>,
    ) {
        let action_rx = self.controls.action_receiver();
        let mut messages = self.message_stream().await;
        let mut quitter = self.db.quitter();
        let mut actions = action_rx.stream();
        let mut about_to_finish = about_to_finish_rx.stream();

        loop {
            select! {
                quit = quitter.recv() => {
                    match quit {
                        Ok(quit) => {
                            if quit {
                                debug!("quitting");
                                break;
                            }
                        },
                        Err(_) => {
                            debug!("quitting, with error");
                            break;
                        },
                    }
                }
                Some(almost_done) = about_to_finish.next() => {
                    if almost_done {
                        if let Some(url) = self.prep_next_track().await {
                            next_track_tx.send(url).expect("failed to send next track url");
                        }
                    }
                }
                Some(action) = actions.next() => {
                    match action {
                        Action::JumpBackward => self.jump_backward().await,
                        Action::JumpForward => self.jump_forward().await,
                        Action::Next => self.skip_forward(None).await.expect("failed to skip forward"),
                        Action::Pause => self.pause(),
                        Action::Play => self.play(),
                        Action::PlayPause => self.play_pause(),
                        Action::Previous => self.skip_backward(None).await.expect("failed to skip forward"),
                        Action::Stop => self.stop(),
                        Action::PlayAlbum { album } => self.play_album(*album, None).await,
                        Action::PlayTrack { track } => self.play_track(*track, None).await,
                        Action::PlayUri { uri } => self.play_uri(uri, Some(self.client.quality())).await,
                        Action::PlayPlaylist { playlist } => self.play_playlist(*playlist, Some(self.client.quality())).await,
                        Action::Quit => self.db.quit(),
                        Action::SkipTo { num } => self.skip_to(num).await.expect("failed to skip to track"),
                        Action::SkipToById { track_id } => self.skip_to_by_id(track_id).await.expect("failed to skip to track"),
                    }
                }
                Some(msg) = messages.next() => {
                    match msg.view() {
                        MessageView::Eos(_) => {
                            debug!("END OF STREAM");

                            self.stop();
                            self.db.quit();
                            break;
                        },
                        MessageView::StreamStart(_) => {
                            self.dbus_metadata_changed().await;
                        }
                        MessageView::DurationChanged(_) => {
                            if let Some(duration) = self.duration() {
                                self.db.insert::<String, ClockValue>(StateKey::Player(PlayerKey::Duration), duration.clone()).await;
                            }

                        }
                        MessageView::Buffering(_) => {
                            debug!("buffering");
                            self.is_buffering = true;
                        }
                        MessageView::AsyncDone(_) => {
                            // If the player is resuming from a previous session,
                            // seek to the last position saved to the state.
                            if self.resume {
                                if let Some(position) = self.db.get::<String, ClockValue>(StateKey::Player(PlayerKey::Position)).await {
                                    self.resume = false;
                                    self.seek(position, None).await.expect("seek failure");
                                }
                            }

                        }
                        MessageView::StateChanged(state_changed) => {
                            if state_changed
                                .src()
                                .map(|s| s == self.playbin)
                                .unwrap_or(false)
                            {
                                let current_state = state_changed
                                    .current()
                                    .to_value()
                                    .get::<GstState>()
                                    .unwrap();

                                let iface_ref = self.player_iface().await;
                                let iface = iface_ref.get_mut().await;

                                match current_state {
                                    GstState::Playing => {
                                        debug!("player state changed to Playing");
                                        self.is_buffering = false;

                                        self.db.insert::<String, StatusValue>(StateKey::Player(PlayerKey::Status),GstState::Playing.into()).await;

                                        iface
                                            .playback_status_changed(iface_ref.signal_context())
                                            .await
                                            .expect("failed");

                                    }
                                    GstState::Paused => {
                                        debug!("player state changed to Paused");
                                        self.is_buffering = false;

                                        self.db.insert::<String, StatusValue>(StateKey::Player(PlayerKey::Status),GstState::Paused.into()).await;

                                        iface
                                            .playback_status_changed(iface_ref.signal_context())
                                            .await
                                            .expect("failed");
                                    }
                                    GstState::Ready => {
                                        debug!("player state changed to Ready");
                                        self.is_buffering = false;

                                        self.db.insert::<String, StatusValue>(StateKey::Player(PlayerKey::Status),GstState::Ready.into()).await;

                                        iface
                                            .playback_status_changed(iface_ref.signal_context())
                                            .await
                                            .expect("failed");

                                    }
                                    GstState::VoidPending => {
                                        debug!("player state changed to VoidPending");
                                        self.is_buffering = false;

                                        self.db.insert::<String, StatusValue>(StateKey::Player(PlayerKey::Status),GstState::Ready.into()).await;

                                        iface
                                            .playback_status_changed(iface_ref.signal_context())
                                            .await
                                            .expect("failed");

                                    },
                                    GstState::Null => {
                                        debug!("player state changed to Null");
                                        self.is_buffering = false;

                                        self.db.insert::<String, StatusValue>(StateKey::Player(PlayerKey::Status),GstState::Ready.into()).await;

                                        iface
                                            .playback_status_changed(iface_ref.signal_context())
                                            .await
                                            .expect("failed");
                                    },
                                    _ => (),

                                }
                            }
                        }
                        MessageView::Error(err) => {
                            println!(
                                "Error from {:?}: {} ({:?})",
                                err.src().map(|s| s.path_string()),
                                err.error(),
                                err.debug()
                            );
                            break;
                        }
                        _ => (),
                    }

                }
            }
        }
    }
    /// Inserts the most recent position, duration and progress values into the state
    /// at a set interval.
    async fn clock_loop(&self) {
        let mut quit_receiver = self.db.quitter();

        loop {
            if let Ok(quit) = quit_receiver.try_recv() {
                if quit {
                    debug!("quitting");
                    break;
                }
            }
            if self.current_state() != GstState::VoidPending.into()
                || self.current_state() != GstState::Null.into()
            {
                if let (Some(position), Some(duration)) = (self.position(), self.duration()) {
                    self.db
                        .insert::<String, ClockValue>(
                            StateKey::Player(PlayerKey::Position),
                            position.clone(),
                        )
                        .await;

                    self.db
                        .insert::<String, ClockValue>(
                            StateKey::Player(PlayerKey::Duration),
                            duration.clone(),
                        )
                        .await;

                    if position >= ClockTime::from_seconds(0).into() && position <= duration {
                        let duration = duration.inner_clocktime();
                        let position = position.inner_clocktime();

                        let remaining = duration - position;
                        let progress = position.seconds() as f64 / duration.seconds() as f64;

                        self.db
                            .insert::<String, FloatValue>(
                                StateKey::Player(PlayerKey::Progress),
                                progress.into(),
                            )
                            .await;

                        self.db
                            .insert::<String, ClockValue>(
                                StateKey::Player(PlayerKey::DurationRemaining),
                                remaining.into(),
                            )
                            .await;
                    }
                }

                std::thread::sleep(Duration::from_millis(REFRESH_RESOLUTION));
            }
        }
    }
    async fn dbus_track_list_replaced_signal(&self, tracks: Vec<String>, current: String) {
        let object_server = self.connection.object_server();

        let iface_ref = object_server
            .interface::<_, MprisTrackList>("/org/mpris/MediaPlayer2")
            .await
            .expect("failed to get object server");

        MprisTrackList::track_list_replaced(iface_ref.signal_context(), tracks, current)
            .await
            .expect("failed to send track list replaced signal");
    }
    async fn player_iface(&self) -> zbus::InterfaceRef<MprisPlayer> {
        let object_server = self.connection.object_server();

        object_server
            .interface::<_, MprisPlayer>("/org/mpris/MediaPlayer2")
            .await
            .expect("failed to get object server")
    }
    async fn dbus_seeked_signal(&self, position: ClockValue) {
        let iface_ref = self.player_iface().await;

        mpris::MprisPlayer::seeked(
            iface_ref.signal_context(),
            position.inner_clocktime().useconds() as i64,
        )
        .await
        .expect("failed to send seeked signal");
    }

    async fn dbus_metadata_changed(&self) {
        let iface_ref = self.player_iface().await;
        let iface = iface_ref.get_mut().await;

        iface
            .metadata_changed(iface_ref.signal_context())
            .await
            .expect("failed to signal metadata change");
    }
    /// Sets up basic functionality for the player.
    async fn prep_next_track(&mut self) -> Option<String> {
        let mut playlist = self.tracklist.lock().await;
        let mut prev_playlist = self.previous_tracklist.lock().await;

        if let Some(mut next_track) = playlist.pop_front() {
            debug!("received new track, adding to player");
            if let Ok(next_playlist_track_url) =
                self.client.track_url(next_track.track.id, None, None).await
            {
                if let Some(previous_track) = self
                    .db
                    .get::<String, TrackListTrack>(StateKey::Player(PlayerKey::NextUp))
                    .await
                {
                    prev_playlist.push_back(previous_track);
                }

                next_track.set_track_url(next_playlist_track_url.clone());

                self.db
                    .insert::<String, TrackListTrack>(
                        StateKey::Player(PlayerKey::NextUp),
                        next_track.clone(),
                    )
                    .await;

                self.db
                    .insert::<String, TrackListValue>(
                        StateKey::Player(PlayerKey::Playlist),
                        playlist.clone(),
                    )
                    .await;

                self.db
                    .insert::<String, TrackListValue>(
                        StateKey::Player(PlayerKey::PreviousPlaylist),
                        prev_playlist.clone(),
                    )
                    .await;

                self.dbus_metadata_changed().await;

                Some(next_playlist_track_url.url)
            } else {
                None
            }
        } else {
            debug!("no more tracks left");
            None
        }
    }
    /// Attach a `TrackURL` to the given track.
    pub async fn attach_track_url(&self, mut track: TrackListTrack) -> Result<TrackListTrack> {
        if let Ok(track_url) = self.client.track_url(track.track.id, None, None).await {
            Ok(track.set_track_url(track_url))
        } else {
            Err(Error::TrackURL)
        }
    }

    /// Get Gstreamer message stream
    pub async fn message_stream(&self) -> BusStream {
        self.playbin.bus().unwrap().stream()
    }
}

/// Provides controls for other modules to send commands
/// to the player
#[derive(Debug, Clone)]
pub struct Controls {
    action_tx: Sender<Action>,
    action_rx: Receiver<Action>,
    db: Database,
}

impl Controls {
    fn new(db: Database) -> Controls {
        let (action_tx, action_rx) = flume::bounded::<Action>(10);

        Controls {
            action_rx,
            action_tx,
            db,
        }
    }
    pub fn action_receiver(&self) -> Receiver<Action> {
        self.action_rx.clone()
    }
    pub async fn play(&self) {
        action!(self, Action::Play);
    }
    pub async fn pause(&self) {
        action!(self, Action::Pause);
    }
    pub async fn play_pause(&self) {
        action!(self, Action::PlayPause);
    }
    pub async fn stop(&self) {
        action!(self, Action::Stop);
    }
    pub async fn quit(&self) {
        action!(self, Action::Quit)
    }
    pub async fn next(&self) {
        action!(self, Action::Next);
    }
    pub async fn previous(&self) {
        action!(self, Action::Previous);
    }
    pub async fn skip_to(&self, num: usize) {
        action!(self, Action::SkipTo { num });
    }
    pub async fn skip_to_by_id(&self, track_id: usize) {
        action!(self, Action::SkipToById { track_id })
    }
    pub async fn jump_forward(&self) {
        action!(self, Action::JumpForward);
    }
    pub async fn jump_backward(&self) {
        action!(self, Action::JumpBackward);
    }
    pub async fn play_album(&self, album: Album) {
        action!(
            self,
            Action::PlayAlbum {
                album: Box::new(album)
            }
        );
    }
    pub async fn play_uri(&self, uri: String) {
        action!(self, Action::PlayUri { uri });
    }
    pub async fn play_track(&self, track: Track) {
        action!(
            self,
            Action::PlayTrack {
                track: Box::new(track)
            }
        );
    }
    pub async fn play_playlist(&self, playlist: Playlist) {
        action!(
            self,
            Action::PlayPlaylist {
                playlist: Box::new(playlist)
            }
        )
    }
    pub async fn position(&self) -> Option<ClockValue> {
        self.db
            .get::<String, ClockValue>(StateKey::Player(PlayerKey::Position))
            .await
    }
    pub async fn status(&self) -> Option<StatusValue> {
        self.db
            .get::<String, StatusValue>(StateKey::Player(PlayerKey::Status))
            .await
    }
    pub async fn currently_playing_track(&self) -> Option<TrackListTrack> {
        self.db
            .get::<String, TrackListTrack>(StateKey::Player(PlayerKey::NextUp))
            .await
    }

    pub async fn remaining_tracks(&self) -> Option<TrackListValue> {
        self.db
            .get::<String, TrackListValue>(StateKey::Player(PlayerKey::Playlist))
            .await
    }
}

#[macro_export]
macro_rules! action {
    ($self:ident, $action:expr) => {
        if let Err(_) = $self.action_tx.send_async($action).await {
            error!("error sending action");
        }
    };
}
