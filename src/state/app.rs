use super::{HifiDB, StateTree};
use std::path::PathBuf;

pub enum AppKey {
    Client(ClientKey),
    Player(PlayerKey),
}

impl AppKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppKey::Client(key) => key.as_str(),
            AppKey::Player(key) => key.as_str(),
        }
    }
}

#[non_exhaustive]
pub enum ClientKey {
    ActiveSecret,
    AppID,
    DefaultQuality,
    Password,
    Token,
    Username,
}

impl ClientKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClientKey::ActiveSecret => "active_secret",
            ClientKey::AppID => "app_id",
            ClientKey::DefaultQuality => "default_quality",
            ClientKey::Password => "password",
            ClientKey::Token => "token",
            ClientKey::Username => "username",
        }
    }
}

#[non_exhaustive]
pub enum PlayerKey {
    Duration,
    DurationRemaining,
    NextUp,
    Playlist,
    Position,
    PreviousPlaylist,
    Progress,
    Status,
}

impl PlayerKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlayerKey::Duration => "duration",
            PlayerKey::DurationRemaining => "duration_remaining",
            PlayerKey::NextUp => "next_up",
            PlayerKey::Playlist => "playlist",
            PlayerKey::Position => "position",
            PlayerKey::PreviousPlaylist => "prev_playlist",
            PlayerKey::Progress => "progress",
            PlayerKey::Status => "status",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub player: StateTree,
    pub config: StateTree,
    db: HifiDB,
}

pub fn new(base_dir: PathBuf) -> AppState {
    let mut db_dir = base_dir;
    db_dir.push("database");

    let db = HifiDB(
        sled::Config::default()
            .path(db_dir)
            .open()
            .expect("could not open database"),
    );

    AppState {
        config: db.open_tree("config"),
        player: db.open_tree("player"),
        db,
    }
}

impl AppState {
    pub fn flush(&self) {
        self.db.0.flush().expect("ERROR: Failed to flush database.");
    }
}
