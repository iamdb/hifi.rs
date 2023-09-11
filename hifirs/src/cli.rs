use std::{net::SocketAddr, time::Duration};

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
use dialoguer::{Confirm, Input, Password};
use hifirs_qobuz_api::client::{api::OutputFormat, AudioQuality};
use snafu::prelude::*;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{fmt, prelude::*};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Provide a username. (overrides any database value)
    #[clap(short, long)]
    pub username: Option<String>,

    #[clap(short, long)]
    /// Provide a password. (overrides any database value)
    pub password: Option<String>,

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
    /// (default 0.0.0.0:9888)
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
    username: Option<String>,
    password: Option<String>,
    resume: bool,
    web: bool,
    interface: SocketAddr,
) -> Result<(), Error> {
    player::init(username, password, quit_when_done).await?;

    if resume {
        tokio::spawn(async move {
            match player::resume(false).await {
                Ok(_) => debug!("resume success"),
                Err(error) => debug!("resume error {error}"),
            }
        });
    }

    #[cfg(target_os = "linux")]
    {
        let controls = player::controls();
        let conn = mpris::init(controls).await;

        tokio::spawn(async {
            mpris::receive_notifications(conn).await;
        });
    }

    if web {
        tokio::spawn(async move { websocket::init(interface).await });
    }

    tokio::spawn(async { player::player_loop().await });

    Ok(())
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
            setup_player(
                cli.quit_when_done,
                cli.username.to_owned(),
                cli.password.to_owned(),
                true,
                cli.web,
                cli.interface,
            )
            .await?;

            wait!(cli.disable_tui);

            Ok(())
        }
        Commands::Play { url } => {
            setup_player(
                cli.quit_when_done,
                cli.username.to_owned(),
                cli.password.to_owned(),
                false,
                cli.web,
                cli.interface,
            )
            .await?;

            player::play_uri(url).await?;

            wait!(cli.disable_tui);

            Ok(())
        }
        Commands::StreamTrack { track_id } => {
            setup_player(
                cli.quit_when_done,
                cli.username.to_owned(),
                cli.password.to_owned(),
                false,
                cli.web,
                cli.interface,
            )
            .await?;

            player::play_track(track_id).await?;

            wait!(cli.disable_tui);

            Ok(())
        }
        Commands::StreamAlbum { album_id } => {
            setup_player(
                cli.quit_when_done,
                cli.username.to_owned(),
                cli.password.to_owned(),
                false,
                cli.web,
                cli.interface,
            )
            .await?;

            player::play_album(album_id).await?;

            wait!(cli.disable_tui);

            Ok(())
        }
        Commands::Api { command } => match command {
            ApiCommands::Search {
                query,
                limit,
                output_format,
            } => {
                let client = qobuz::make_client(cli.username, cli.password).await?;
                let results = client.search_all(query, limit.unwrap_or_default()).await?;

                output!(results, output_format);

                Ok(())
            }
            ApiCommands::SearchAlbums {
                query,
                limit,
                output_format,
            } => {
                let client = qobuz::make_client(cli.username, cli.password).await?;
                let results = client.search_albums(query.clone(), limit).await?;

                output!(results, output_format);

                Ok(())
            }
            ApiCommands::SearchArtists {
                query,
                limit,
                output_format,
            } => {
                let client = qobuz::make_client(cli.username, cli.password).await?;
                let results = client.search_artists(query.clone(), limit).await?;

                output!(results, output_format);

                Ok(())
            }
            ApiCommands::Playlist { id, output_format } => {
                let client = qobuz::make_client(cli.username, cli.password).await?;

                let results = client.playlist(id).await?;
                output!(results, output_format);
                Ok(())
            }
            ApiCommands::Album { id, output_format } => {
                let client = qobuz::make_client(cli.username, cli.password).await?;

                let results = client.album(&id).await?;
                output!(results, output_format);
                Ok(())
            }
            ApiCommands::Artist { id, output_format } => {
                let client = qobuz::make_client(cli.username, cli.password).await?;

                let results = client.artist(id, Some(500)).await?;
                output!(results, output_format);
                Ok(())
            }
            ApiCommands::Track { id, output_format } => {
                let client = qobuz::make_client(cli.username, cli.password).await?;

                let results = client.track(id).await?;
                output!(results, output_format);
                Ok(())
            }
        },
        Commands::Reset => {
            db::clear_state().await;
            Ok(())
        }
        Commands::Config { command } => match command {
            ConfigCommands::Username {} => {
                if let Ok(username) = Input::new()
                    .with_prompt("Enter your username / email")
                    .interact_text()
                {
                    db::set_username(username).await;

                    println!("Username saved.");
                }
                Ok(())
            }
            ConfigCommands::Password {} => {
                if let Ok(password) = Password::new()
                    .with_prompt("Enter your password (hidden)")
                    .interact()
                {
                    let md5_pw = format!("{:x}", md5::compute(password));

                    debug!("saving password to database: {}", md5_pw);

                    db::set_password(md5_pw).await;

                    println!("Password saved.");
                }
                Ok(())
            }
            ConfigCommands::DefaultQuality { quality } => {
                db::set_default_quality(quality).await;

                println!("Default quality saved.");

                Ok(())
            }
            ConfigCommands::Clear {} => {
                if let Ok(ok) = Confirm::new()
                    .with_prompt("This will clear the configuration in the database.\nDo you want to continue?")
                    .interact()
                {
                    if ok {
                        db::clear_state().await;
                        println!("Database cleared.");
                    }
                }
                Ok(())
            }
        },
    }
}

#[macro_export]
macro_rules! wait {
    ($disable_tui: expr) => {
        if !$disable_tui {
            let mut tui = CursiveUI::new();
            tokio::spawn(async { cursive::receive_notifications().await });
            tui.run().await;

            debug!("tui exited, quitting");
            player::controls().quit().await;

            std::thread::sleep(Duration::from_millis(500));
        } else {
            debug!("waiting for ctrlc");
            tokio::signal::ctrl_c()
                .await
                .expect("error waiting for ctrlc");

            debug!("ctrlc received, quitting");
            player::controls().quit().await;

            std::thread::sleep(Duration::from_millis(500));
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
