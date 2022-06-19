mod cli;
mod qobuz;
mod utils;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;
use crate::cli::{Cli, Commands};
use crate::qobuz::client;
use clap::Parser;
use rocksdb::{DBWithThreadMode, SingleThreaded, DB};

pub type QobuzDB = DBWithThreadMode<SingleThreaded>;

fn main() {
    pretty_env_logger::init();
    let cli = Cli::parse();

    let path = "db";
    let db: QobuzDB = DB::open_default(path).expect("could not open database");
    let mut client = client::new();

    // Config auth
    if let Some(token) = db.get(b"token").unwrap() {
        debug!("using token from cache");
        client.set_token(String::from_utf8(token).unwrap());
    } else {
        if let Some(u) = cli.username {
            debug!("using username from cli argument");
            client.set_username(u);
        } else if let Some(u) = db.get(b"username").unwrap() {
            debug!("using username stored in database");
            client.set_username(String::from_utf8(u).unwrap());
        }

        if let Some(p) = cli.password {
            debug!("using password from cli argument");
            client.set_password(p);
        } else if let Some(p) = db.get(b"password").unwrap() {
            debug!("using password stored in database");
            client.set_password(String::from_utf8(p).unwrap());
        }
    }
    // CLI Commands
    match &cli.command {
        Commands::SearchAlbums { query } => {
            client.check_auth();
            if let Some(results) = client.search_albums(query) {
                let json = serde_json::to_string(&results);
                print!("{}", json.unwrap());
            }
        }
        Commands::GetAlbum { id } => {
            client.check_auth();
            if let Some(results) = client.album(id) {
                let json = serde_json::to_string(&results);
                print!("{}", json.unwrap());
            }
        }
        Commands::SearchArtists { query } => {
            client.check_auth();
            if let Some(results) = client.search_artists(query) {
                let json = serde_json::to_string(&results);
                print!("{}", json.unwrap());
            }
        }
        Commands::GetArtist { id } => {
            client.check_auth();
            if let Some(results) = client.artist(id) {
                let json = serde_json::to_string(&results);
                print!("{}", json.unwrap());
            }
        }
        Commands::GetTrack { id } => {
            client.check_auth();
            if let Some(results) = client.track(id) {
                let json = serde_json::to_string(&results);
                print!("{}", json.unwrap());
            }
        }
        Commands::TrackURL { id, quality } => {
            client.check_auth();

            if let Ok(result) = client.track_url(id, quality, None) {
                let json = serde_json::to_string(&result).unwrap();
                print!("{}", json);
            }
        }

        Commands::Config(config) => {
            if let Some(password) = &config.set_password {
                db.put(b"password", password).unwrap();
            }

            if let Some(username) = &config.set_username {
                db.put(b"username", username).unwrap();
            }

            debug!("{:?}", config);
        }
    }
}
