use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    /// Provide a username. (overrides any database value)
    #[clap(short, long)]
    pub username: Option<String>,
    #[clap(short, long)]
    /// Provide a password. (overrides any database value)
    pub password: Option<String>,
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Search for albums in the Qobuz database
    SearchAlbums {
        #[clap(value_parser)]
        query: String,
    },
    GetAlbum {
        #[clap(value_parser)]
        id: String,
    },
    /// Search for artists in the Qobuz database
    SearchArtists {
        #[clap(value_parser)]
        query: String,
    },
    GetArtist {
        #[clap(value_parser)]
        id: String,
    },
    GetTrack {
        #[clap(value_parser)]
        id: String,
    },
    TrackURL {
        #[clap(value_parser)]
        id: i32,
        #[clap(value_parser)]
        quality: i32,
    },
    /// Set configuration options
    Config(Config),
}

#[derive(Args, Debug)]
pub struct Config {
    #[clap(long)]
    /// Store the username for automatic login
    pub set_username: Option<String>,
    #[clap(long)]
    /// Store the password for automatic login
    pub set_password: Option<String>,
}
