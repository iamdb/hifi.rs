#[cfg(target_os = "linux")]
use crate::mpris;
use crate::{
    cursive::{self, CursiveUI},
    player::{self, Player},
    qobuz::{self, SearchResults},
    sql::db::{self, Database},
    state::app::PlayerState,
};
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Table};
use dialoguer::{Confirm, Input, Password};
use hifirs_qobuz_api::client::{api::OutputFormat, AudioQuality};
use snafu::prelude::*;
use std::sync::Arc;
use tokio::sync::RwLock;
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
    #[clap(short, long)]
    /// Quit after done playing
    pub quit_when_done: Option<bool>,

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

async fn setup_player<'s>(
    database: Database,
    quit_when_done: bool,
    username: Option<String>,
    password: Option<String>,
    resume: bool,
) -> Result<(Arc<RwLock<Player>>, CursiveUI), Error> {
    let client = qobuz::make_client(username, password, &database).await?;
    let state = Arc::new(RwLock::new(PlayerState::new(client.clone(), database)));

    let new_player = player::new(client.clone(), state.clone(), quit_when_done).await?;

    let notify_receiver = new_player.notify_receiver();
    let safe_player = new_player.safe();

    let controls = safe_player.read().await.controls();
    let tui = CursiveUI::new(controls, client.clone());

    if resume {
        let s = safe_player.clone();
        tokio::spawn(async move {
            s.write()
                .await
                .resume(false)
                .await
                .expect("failed to resume");
        });
    }

    #[cfg(target_os = "linux")]
    {
        let controls = safe_player.read().await.controls();
        let conn = mpris::init(controls).await;

        let nr = notify_receiver.clone();
        let s = state.clone();
        tokio::spawn(async {
            mpris::receive_notifications(s, conn, nr).await;
        });
    }

    let sink = tui.sink().await.clone();
    tokio::spawn(async { cursive::receive_notifications(sink, notify_receiver).await });

    let p = safe_player.clone();
    tokio::spawn(async { player::player_loop(p, client, state).await });

    Ok((safe_player, tui))
}

pub async fn run() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(fmt::layer().pretty().with_writer(std::io::stderr))
        .with(EnvFilter::from_env("HIFIRS_LOG"))
        .init();
    //pretty_env_logger::init();
    // PARSE CLI ARGS
    let cli = Cli::parse();

    // DATABASE DIRECTORY
    let mut base_dir = dirs::data_local_dir().unwrap();
    base_dir.push("hifi-rs");

    // SETUP DATABASE
    let data = db::new().await;

    let mut quit_when_done = false;

    if let Some(should_quit) = cli.quit_when_done {
        quit_when_done = should_quit;
    }

    // CLI COMMANDS
    match cli.command {
        Commands::Open {} => {
            let (_player, mut tui) = setup_player(
                data.to_owned(),
                quit_when_done,
                cli.username.to_owned(),
                cli.password.to_owned(),
                true,
            )
            .await?;

            tui.run().await;

            Ok(())
        }
        Commands::Play { url, quality } => {
            let (player, mut tui) = setup_player(
                data.to_owned(),
                quit_when_done,
                cli.username.to_owned(),
                cli.password.to_owned(),
                false,
            )
            .await?;

            player.read_owned().await.play_uri(url, quality).await?;

            tui.run().await;

            Ok(())
        }
        Commands::StreamTrack { track_id, quality } => {
            let (player, mut tui) = setup_player(
                data.to_owned(),
                quit_when_done,
                cli.username.to_owned(),
                cli.password.to_owned(),
                false,
            )
            .await?;

            player
                .read_owned()
                .await
                .play_track(track_id, quality)
                .await?;

            tui.run().await;

            Ok(())
        }
        Commands::StreamAlbum { album_id, quality } => {
            let (player, mut tui) = setup_player(
                data.to_owned(),
                quit_when_done,
                cli.username.to_owned(),
                cli.password.to_owned(),
                false,
            )
            .await?;

            player
                .read_owned()
                .await
                .play_album(album_id, quality)
                .await?;

            tui.run().await;

            Ok(())
        }
        Commands::Api { command } => match command {
            ApiCommands::Search {
                query,
                limit,
                output_format,
            } => {
                let client = qobuz::make_client(cli.username, cli.password, &data).await?;
                let results = client.search_all(query, limit.unwrap_or_default()).await?;

                output!(results, output_format);

                Ok(())
            }
            ApiCommands::SearchAlbums {
                query,
                limit,
                output_format,
            } => {
                let client = qobuz::make_client(cli.username, cli.password, &data).await?;
                let results =
                    SearchResults::Albums(client.search_albums(query.clone(), limit).await?);

                output!(results, output_format);

                Ok(())
            }
            ApiCommands::SearchArtists {
                query,
                limit,
                output_format,
            } => {
                let client = qobuz::make_client(cli.username, cli.password, &data).await?;
                let results =
                    SearchResults::Artists(client.search_artists(query.clone(), limit).await?);

                output!(results, output_format);

                Ok(())
            }
            ApiCommands::Playlist { id, output_format } => {
                let client = qobuz::make_client(cli.username, cli.password, &data).await?;

                let results = client.playlist(id).await?;
                output!(results, output_format);
                Ok(())
            }
            ApiCommands::Album { id, output_format } => {
                let client = qobuz::make_client(cli.username, cli.password, &data).await?;

                let results = client.album(&id).await?;
                output!(results, output_format);
                Ok(())
            }
            ApiCommands::Artist { id, output_format } => {
                let client = qobuz::make_client(cli.username, cli.password, &data).await?;

                let results = client.artist(id, Some(500)).await?;
                output!(results, output_format);
                Ok(())
            }
            ApiCommands::Track { id, output_format } => {
                let client = qobuz::make_client(cli.username, cli.password, &data).await?;

                let results = client.track(id).await?;
                output!(results, output_format);
                Ok(())
            }
        },
        Commands::Reset => {
            data.clear_state().await;
            Ok(())
        }
        Commands::Config { command } => match command {
            ConfigCommands::Username {} => {
                if let Ok(username) = Input::new()
                    .with_prompt("Enter your username / email")
                    .interact_text()
                {
                    data.set_username(username).await;

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

                    data.set_password(md5_pw).await;

                    println!("Password saved.");
                }
                Ok(())
            }
            ConfigCommands::DefaultQuality { quality } => {
                data.set_default_quality(quality).await;

                println!("Default quality saved.");

                Ok(())
            }
            ConfigCommands::Clear {} => {
                if let Ok(ok) = Confirm::new()
                    .with_prompt("This will clear the configuration in the database.\nDo you want to continue?")
                    .interact()
                {
                    if ok {
                        data.clear_state().await;
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
    ($state:expr) => {
        let mut quitter = $state.read().await.quitter();

        let state = $state.clone();
        ctrlc::set_handler(move || {
            state.blocking_read().quit();
            std::process::exit(0);
        })
        .expect("error setting ctrlc handler");

        loop {
            if let Ok(quit) = quitter.try_recv() {
                if quit {
                    debug!("quitting main thread");
                    break;
                }
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
