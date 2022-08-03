use super::{HifiDB, StateTree};
use snafu::prelude::*;
use std::path::PathBuf;
use tokio::sync::broadcast::{Receiver, Sender};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Collection not found."))]
    CollectionNotFound,
    #[snafu(display("Unsupported."))]
    Unsupported,
    #[snafu(display("Reportable bug."))]
    ReportableBug,
    #[snafu(display("Database in use."))]
    Io,
    #[snafu(display("Database corrupted."))]
    Corruption,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

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
    quit_sender: Sender<bool>,
}

pub fn new(base_dir: PathBuf) -> Result<AppState> {
    let mut db_dir = base_dir;
    db_dir.push("database");

    let db = match sled::Config::default()
        .path(db_dir)
        .use_compression(true)
        .compression_factor(10)
        .mode(sled::Mode::LowSpace)
        .print_profile_on_drop(false)
        .open()
    {
        Ok(db) => HifiDB(db),
        Err(err) => match err {
            sled::Error::CollectionNotFound(e) => {
                println!("ERROR: {:?}", e);
                std::process::exit(1);
            }
            sled::Error::Unsupported(e) => {
                println!("ERROR: {}", e);
                std::process::exit(1);
            }
            sled::Error::ReportableBug(_) => {
                println!("ERROR: There is a bug in the database. :(");
                std::process::exit(1);
            }
            sled::Error::Io(_) => {
                println!("The database is in use. Is another session running?");
                std::process::exit(1);
            }
            sled::Error::Corruption { at, bt: _ } => {
                println!("ERROR: Databast corruption at {}", at.unwrap(),);
                std::process::exit(1);
            }
        },
    };

    // Quit channel
    let (quit_sender, _) = tokio::sync::broadcast::channel::<bool>(1);

    warn!("db was recovered: {}", db.0.was_recovered());

    Ok(AppState {
        config: db.open_tree("config"),
        player: db.open_tree("player"),
        quit_sender,
    })
}

impl AppState {
    pub fn quitter(&self) -> Receiver<bool> {
        self.quit_sender.subscribe()
    }

    pub fn send_quit(&self) {
        self.quit_sender
            .send(true)
            .expect("failed to send quit message");
    }
}
