use clap::{Parser, Subcommand};

use crate::state::AudioQuality;

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
    /// Play something interactively
    Play {
        #[clap(value_parser)]
        query: String,
        #[clap(short, long, value_enum)]
        quality: Option<AudioQuality>,
    },
    /// Resume previous session
    Resume {
        #[clap(long, short)]
        no_tui: bool,
    },
    /// Search for tracks, albums, artists and playlists
    Search {
        #[clap(value_parser)]
        query: String,
    },
    /// Search for albums in the Qobuz database
    SearchAlbums {
        #[clap(value_parser)]
        query: String,
    },
    /// Get information for a specific album.
    GetAlbum {
        #[clap(value_parser)]
        id: String,
    },
    /// Search for artists in the Qobuz database
    SearchArtists {
        #[clap(value_parser)]
        query: String,
    },
    /// Get information for a specific artist.
    GetArtist {
        #[clap(value_parser)]
        id: String,
    },
    /// Get information for a specific track.
    GetTrack {
        #[clap(value_parser)]
        id: String,
    },
    /// Get the url needed to play a track.
    TrackURL {
        #[clap(value_parser)]
        id: i32,
        #[clap(short, long, value_enum)]
        quality: Option<AudioQuality>,
    },
    /// Stream an individual track by its ID.
    StreamTrack {
        #[clap(value_parser)]
        track_id: i32,
        #[clap(short, long, value_enum)]
        quality: Option<AudioQuality>,
    },
    /// Stream a full album by its ID.
    StreamAlbum {
        #[clap(value_parser)]
        album_id: String,
        #[clap(short, long, value_enum)]
        quality: Option<AudioQuality>,
        #[clap(short, long)]
        no_tui: bool,
    },
    /// Retreive a list of your playlsits.
    MyPlaylists {},
    /// Retreive information about a specific playlist.
    Playlist { playlist_id: String },
    /// Download a track to disk.
    Download {
        #[clap(value_parser)]
        id: i32,
        #[clap(value_enum)]
        quality: Option<AudioQuality>,
    },
    /// Set configuration options
    Config {
        #[clap(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Save username to database.
    #[clap(value_parser)]
    Username {},
    /// Save password to database.
    #[clap(value_parser)]
    Password {},
    /// Clear saved username and password.
    Clear {},
    /// Target this quality when playing audio.
    DefaultQuality {
        #[clap(value_enum)]
        quality: AudioQuality,
    },
}
