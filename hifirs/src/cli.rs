use std::net::SocketAddr;

#[cfg(target_os = "linux")]
use crate::mpris;
use crate::{
    cursive::{self, CursiveUI},
    player::{self},
    qobuz::{self},
    sql::db::{self},
    wait, websocket,
};
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Table};
use dialoguer::Confirm;
use hifirs_qobuz_api::client::{api::OutputFormat, AudioQuality};
use snafu::prelude::*;
use tokio::task::JoinHandle;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{fmt, prelude::*};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(short, long, default_value_t = false)]
    /// Quit after done playing
    pub quit_when_done: bool,

    #[clap(short, long, default_value_t = false)]
    /// Disable the TUI interface.
    pub disable_tui: bool,

    #[clap(short, long, default_value_t = false)]
    /// Start web server with websocket API and embedded UI.
    pub web: bool,

    #[clap(long, default_value = "0.0.0.0:9888")]
    /// Specify a different interface and port for the web server to listen on.
    pub interface: SocketAddr,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Open the player
    Open {},
    /// Play an Qobuz entity using the url.
    Play {
        #[clap(long, short)]
        url: String,
    },
    /// Stream an individual track by its ID.
    StreamTrack {
        #[clap(value_parser)]
        track_id: i32,
    },
    /// Stream a full album by its ID.
    StreamAlbum {
        #[clap(value_parser)]
        album_id: String,
    },
    /// Retreive data from the Qobuz API
    Api {
        #[clap(subcommand)]
        command: ApiCommands,
    },
    /// Reset the player state
    Reset,
    /// Set configuration options
    Config {
        #[clap(subcommand)]
        command: ConfigCommands,
    },
    /// Authenticate with Qobuz via OAuth (browser-based)
    Oauth {
        #[clap(long)]
        /// Host/IP placed in the redirect URL so other machines can reach the callback.
        /// When set, the callback binds to 0.0.0.0 and no browser is opened automatically.
        advertise: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ApiCommands {
    /// Search for tracks, albums, artists and playlists
    Search {
        #[clap(value_parser)]
        query: String,
        #[clap(long, short)]
        limit: Option<i32>,
        #[clap(short, long = "output", value_enum)]
        output_format: Option<OutputFormat>,
    },
    /// Search for albums in the Qobuz database
    SearchAlbums {
        #[clap(value_parser)]
        query: String,
        #[clap(long, short)]
        limit: Option<i32>,
        #[clap(short, long = "output", value_enum)]
        output_format: Option<OutputFormat>,
    },
    /// Search for artists in the Qobuz database
    SearchArtists {
        #[clap(value_parser)]
        query: String,
        #[clap(long, short)]
        limit: Option<i32>,
        #[clap(short, long = "output", value_enum)]
        output_format: Option<OutputFormat>,
    },
    Album {
        #[clap(value_parser)]
        id: String,
        #[clap(short, long = "output", value_enum)]
        output_format: Option<OutputFormat>,
    },
    Artist {
        #[clap(value_parser)]
        id: i32,
        #[clap(short, long = "output", value_enum)]
        output_format: Option<OutputFormat>,
    },
    Track {
        #[clap(value_parser)]
        id: i32,
        #[clap(short, long = "output", value_enum)]
        output_format: Option<OutputFormat>,
    },
    /// Retreive information about a specific playlist.
    Playlist {
        #[clap(value_parser)]
        id: i64,
        #[clap(short, long = "output", value_enum)]
        output_format: Option<OutputFormat>,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Clear saved configuration and authentication state.
    Clear {},
    /// Target this quality when playing audio.
    DefaultQuality {
        #[clap(value_enum)]
        quality: AudioQuality,
    },
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("{error}"))]
    ClientError { error: String },
    #[snafu(display("{error}"))]
    PlayerError { error: String },
    #[snafu(display("{error}"))]
    TerminalError { error: String },
}

impl From<hifirs_qobuz_api::Error> for Error {
    fn from(error: hifirs_qobuz_api::Error) -> Self {
        Error::ClientError {
            error: error.to_string(),
        }
    }
}

impl From<player::error::Error> for Error {
    fn from(error: player::error::Error) -> Self {
        Error::PlayerError {
            error: error.to_string(),
        }
    }
}

async fn setup_player(
    quit_when_done: bool,
    resume: bool,
    web: bool,
    interface: SocketAddr,
) -> Result<Vec<JoinHandle<()>>, Error> {
    player::init(quit_when_done).await?;

    let mut handles: Vec<JoinHandle<()>> = Vec::new();

    if resume {
        handles.push(tokio::spawn(async move {
            match player::resume(false).await {
                Ok(_) => debug!("resume success"),
                Err(error) => debug!("resume error {error}"),
            }
        }));
    }

    #[cfg(target_os = "linux")]
    {
        let conn = mpris::init().await;

        handles.push(tokio::spawn(async move {
            mpris::receive_notifications(&conn).await;
        }));
    }

    if web {
        handles.push(tokio::spawn(
            async move { websocket::init(interface).await },
        ));
    }

    handles.push(tokio::spawn(async {
        match player::player_loop().await {
            Ok(_) => debug!("player loop exited successfully"),
            Err(error) => debug!("player loop error {error}"),
        }
    }));

    Ok(handles)
}

pub async fn run() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .compact()
                .with_file(false)
                .with_writer(std::io::stderr),
        )
        .with(EnvFilter::from_env("HIFIRS_LOG"))
        .init();

    // PARSE CLI ARGS
    let cli = Cli::parse();

    // INIT DB
    db::init().await;

    // CLI COMMANDS
    match cli.command {
        Commands::Open {} => {
            let mut handles = setup_player(
                cli.quit_when_done,
                true,
                cli.web,
                cli.interface,
            )
            .await?;

            wait!(mut handles, cli.disable_tui);

            Ok(())
        }
        Commands::Play { url } => {
            let mut handles = setup_player(
                cli.quit_when_done,
                false,
                cli.web,
                cli.interface,
            )
            .await?;

            player::play_uri(&url).await?;

            wait!(mut handles, cli.disable_tui);

            Ok(())
        }
        Commands::StreamTrack { track_id } => {
            let mut handles = setup_player(
                cli.quit_when_done,
                false,
                cli.web,
                cli.interface,
            )
            .await?;

            player::play_track(track_id).await?;

            wait!(mut handles, cli.disable_tui);

            Ok(())
        }
        Commands::StreamAlbum { album_id } => {
            let mut handles = setup_player(
                cli.quit_when_done,
                false,
                cli.web,
                cli.interface,
            )
            .await?;

            player::play_album(&album_id).await?;

            wait!(mut handles, cli.disable_tui);

            Ok(())
        }
        Commands::Api { command } => match command {
            ApiCommands::Search {
                query,
                limit,
                output_format,
            } => {
                let client =
                    qobuz::make_client().await?;
                let results = client.search_all(&query, limit.unwrap_or_default()).await?;

                output!(results, output_format);

                Ok(())
            }
            ApiCommands::SearchAlbums {
                query,
                limit,
                output_format,
            } => {
                let client =
                    qobuz::make_client().await?;
                let results = client.search_albums(&query, limit).await?;

                output!(results, output_format);

                Ok(())
            }
            ApiCommands::SearchArtists {
                query,
                limit,
                output_format,
            } => {
                let client =
                    qobuz::make_client().await?;
                let results = client.search_artists(&query, limit).await?;

                output!(results, output_format);

                Ok(())
            }
            ApiCommands::Playlist { id, output_format } => {
                let client =
                    qobuz::make_client().await?;

                let results = client.playlist(id).await?;
                output!(results, output_format);
                Ok(())
            }
            ApiCommands::Album { id, output_format } => {
                let client =
                    qobuz::make_client().await?;

                let results = client.album(&id).await?;
                output!(results, output_format);
                Ok(())
            }
            ApiCommands::Artist { id, output_format } => {
                let client =
                    qobuz::make_client().await?;

                let results = client.artist(id, Some(500)).await?;
                output!(results, output_format);
                Ok(())
            }
            ApiCommands::Track { id, output_format } => {
                let client =
                    qobuz::make_client().await?;

                let results = client.track(id).await?;
                output!(results, output_format);
                Ok(())
            }
        },
        Commands::Reset => {
            db::clear_state().await;
            Ok(())
        }
        Commands::Oauth { advertise } => {
            qobuz::oauth_login(advertise.as_deref()).await?;

            println!("Signed in successfully.");

            Ok(())
        }
        Commands::Config { command } => match command {
            ConfigCommands::DefaultQuality { quality } => {
                db::set_default_quality(quality).await;

                println!("Default quality saved.");

                Ok(())
            }
            ConfigCommands::Clear {} => {
                if let Ok(ok) = Confirm::new()
                    .with_prompt("This will clear the saved authentication and configuration.\nDo you want to continue?")
                    .interact()
                {
                    if ok {
                        db::clear_config().await;
                        println!("Configuration cleared.");
                    }
                }
                Ok(())
            }
        },
    }
}

#[macro_export]
macro_rules! wait {
    (mut $handles: expr, $disable_tui: expr) => {
        if !$disable_tui {
            let mut tui = CursiveUI::new();

            $handles.push(tokio::spawn(async {
                cursive::receive_notifications().await
            }));

            tui.run().await;

            debug!("tui exited, quitting");
            player::quit().await?;

            for h in $handles {
                match h.await {
                    Ok(_) => debug!("task exited"),
                    Err(error) => debug!("task error {error}"),
                };
            }
        } else {
            debug!("waiting for ctrlc");
            tokio::signal::ctrl_c()
                .await
                .expect("error waiting for ctrlc");

            debug!("ctrlc received, quitting");
            player::quit().await?;

            for h in $handles {
                match h.await {
                    Ok(_) => debug!("task exited"),
                    Err(error) => debug!("task error {error}"),
                };
            }
        }
    };
}

#[macro_export]
macro_rules! output {
    ($results:ident, $output_format:expr) => {
        match $output_format {
            Some(OutputFormat::Json) => {
                let json =
                    serde_json::to_string(&$results).expect("failed to convert results to string");

                print!("{}", json);
            }
            Some(OutputFormat::Tsv) => {
                // let formatted_results: Vec<Vec<String>> = $results.into();

                // let rows = formatted_results
                //     .iter()
                //     .map(|row| {
                //         let tabbed = row.join("\t");

                //         tabbed
                //     })
                //     .collect::<Vec<String>>();

                print!("");
            }
            None => {
                let mut table = Table::new();
                table.load_preset(UTF8_FULL);
                table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);

                //let table_rows: Vec<Vec<String>> = $results.into();

                // for row in table_rows {
                //     table.add_row(row);
                // }

                print!("{}", table);
            }
        }
    };
}

pub(crate) use output;
