use crate::{
    player,
    qobuz::{self, SearchResults},
    sql::db,
    switch_screen, ui, wait,
};
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Table};
use dialoguer::{Confirm, Input, Password};
use hifirs_qobuz_api::client::{api::OutputFormat, AudioQuality};
use snafu::prelude::*;

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
    /// Resume previous session
    Resume {
        #[clap(long, short)]
        no_tui: bool,
    },
    Play {
        #[clap(long, short)]
        no_tui: bool,
        #[clap(long, short)]
        uri: String,
    },
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
        #[clap(short, long = "output", value_enum)]
        output_format: Option<OutputFormat>,
        #[clap(long, short)]
        no_tui: bool,
        #[clap(long, short)]
        limit: Option<i32>,
    },
    /// Search for artists in the Qobuz database
    SearchArtists {
        #[clap(value_parser)]
        query: String,
        #[clap(short, long = "output", value_enum)]
        output_format: Option<OutputFormat>,
        #[clap(long, short)]
        no_tui: bool,
        #[clap(long, short)]
        limit: Option<i32>,
    },
    /// Stream an individual track by its ID.
    StreamTrack {
        #[clap(value_parser)]
        track_id: i32,
        #[clap(short, long, value_enum)]
        quality: Option<AudioQuality>,
        #[clap(short, long)]
        no_tui: bool,
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
    MyPlaylists {
        #[clap(short, long = "output", value_enum)]
        output_format: Option<OutputFormat>,
        #[clap(short, long)]
        no_tui: bool,
    },
    /// Retreive information about a specific playlist.
    Playlist {
        playlist_id: i64,
        output_format: Option<OutputFormat>,
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
    #[snafu(display("Client Error: {error}"))]
    ClientError {
        error: hifirs_qobuz_api::Error,
    },
    PlayerError {
        error: player::Error,
    },
    TerminalError {
        error: ui::Error,
    },
}

impl From<hifirs_qobuz_api::Error> for Error {
    fn from(error: hifirs_qobuz_api::Error) -> Self {
        Error::ClientError { error }
    }
}

impl From<player::Error> for Error {
    fn from(error: player::Error) -> Self {
        Error::PlayerError { error }
    }
}

impl From<ui::Error> for Error {
    fn from(error: ui::Error) -> Self {
        Error::TerminalError { error }
    }
}

pub async fn run() -> Result<(), Error> {
    pretty_env_logger::init();
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
        Commands::Resume { no_tui } => {
            let client = qobuz::make_client(cli.username, cli.password, &data).await?;
            let mut player = player::new(client.clone(), data, quit_when_done).await?;

            player.resume(true).await?;

            if no_tui {
                wait!(player.state());
            } else {
                let mut tui =
                    ui::new(player.state(), player.controls(), client, None, None).await?;
                tui.event_loop().await?;
            }

            Ok(())
        }
        Commands::Play { uri, no_tui } => {
            let client = qobuz::make_client(cli.username, cli.password, &data).await?;
            let player = player::new(client.clone(), data, quit_when_done).await?;

            player.play_uri(uri, Some(client.quality())).await?;

            if no_tui {
                wait!(player.state());
            } else {
                let mut tui =
                    ui::new(player.state(), player.controls(), client, None, None).await?;
                tui.event_loop().await?;
            }
            Ok(())
        }
        #[allow(unused_variables)]
        Commands::Search {
            query,
            limit,
            output_format,
        } => {
            let client = qobuz::make_client(cli.username, cli.password, &data).await?;

            match client.search_all(query).await {
                Ok(results) => {
                    //let json = serde_json::to_string(&results);
                    //print!("{}", results);
                    Ok(())
                }
                Err(error) => Err(Error::ClientError { error }),
            }
        }
        Commands::SearchAlbums {
            query,
            limit,
            output_format,
            no_tui,
        } => {
            let client = qobuz::make_client(cli.username, cli.password, &data).await?;

            let results = SearchResults::Albums(client.search_albums(query.clone(), limit).await?);

            if no_tui {
                output!(results, output_format);
            } else {
                let mut player = player::new(client.clone(), data, quit_when_done).await?;
                player.resume(false).await?;

                if no_tui {
                    wait!(player.state());
                } else {
                    let mut tui = ui::new(
                        player.state(),
                        player.controls(),
                        client,
                        Some(results),
                        Some(query),
                    )
                    .await?;
                    tui.event_loop().await?;
                }
            }

            Ok(())
        }
        Commands::SearchArtists {
            query,
            limit,
            output_format,
            no_tui,
        } => {
            let client = qobuz::make_client(cli.username, cli.password, &data).await?;

            let results =
                SearchResults::Artists(client.search_artists(query.clone(), limit).await?);

            if no_tui {
                output!(results, output_format);
            } else {
                let mut player = player::new(client.clone(), data, quit_when_done).await?;
                player.resume(false).await?;

                if no_tui {
                    wait!(player.state());
                } else {
                    let mut tui = ui::new(
                        player.state(),
                        player.controls(),
                        client,
                        Some(results),
                        Some(query),
                    )
                    .await?;
                    tui.event_loop().await?;
                }
            }

            Ok(())
        }
        Commands::MyPlaylists {
            no_tui,
            output_format,
        } => {
            let client = qobuz::make_client(cli.username, cli.password, &data).await?;

            if output_format.is_some() {
                if let Ok(playlists) = client.user_playlists().await {
                    let results = SearchResults::UserPlaylists(playlists);
                    output!(results, output_format)
                }
            } else {
                let mut player = player::new(client.clone(), data, quit_when_done).await?;
                player.resume(false).await?;

                if no_tui {
                    wait!(player.state());
                } else {
                    let mut tui =
                        ui::new(player.state(), player.controls(), client, None, None).await?;

                    let state = player.state();

                    switch_screen!(state.write().await, ActiveScreen::Playlists);
                    tui.event_loop().await?;
                }
            }

            Ok(())
        }
        Commands::Playlist {
            playlist_id,
            output_format,
        } => {
            let client = qobuz::make_client(cli.username, cli.password, &data).await?;

            let results = SearchResults::Playlist(Box::new(client.playlist(playlist_id).await?));
            output!(results, output_format);
            Ok(())
        }
        Commands::StreamTrack {
            track_id,
            quality,
            no_tui,
        } => {
            let client = qobuz::make_client(cli.username, cli.password, &data).await?;

            let player = player::new(client.clone(), data, quit_when_done).await?;

            let track = client.track(track_id).await?;

            player.play_track(track, Some(quality.unwrap())).await?;

            if no_tui {
                wait!(player.state());
            } else {
                let mut tui =
                    ui::new(player.state(), player.controls(), client, None, None).await?;
                tui.event_loop().await?;
            }

            Ok(())
        }
        Commands::StreamAlbum {
            album_id,
            quality,
            no_tui,
        } => {
            let client = qobuz::make_client(cli.username, cli.password, &data).await?;

            let album = client.album(album_id).await?;

            let quality = if let Some(q) = quality {
                q
            } else {
                client.quality()
            };

            let player = player::new(client.clone(), data, quit_when_done).await?;
            player.play_album(album, Some(quality)).await?;

            if no_tui {
                wait!(player.state());
            } else {
                let mut tui =
                    ui::new(player.state(), player.controls(), client, None, None).await?;

                let state = player.state();
                switch_screen!(state.write().await, ActiveScreen::NowPlaying);

                tui.event_loop().await?;
            }

            Ok(())
        }
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
                let formatted_results: Vec<Vec<String>> = $results.into();

                let rows = formatted_results
                    .iter()
                    .map(|row| {
                        let tabbed = row.join("\t");

                        tabbed
                    })
                    .collect::<Vec<String>>();

                print!("{}", rows.join("\n"));
            }
            None => {
                let mut table = Table::new();
                table.load_preset(UTF8_FULL);
                table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
                table.set_header($results.headers());

                let table_rows: Vec<Vec<String>> = $results.into();

                for row in table_rows {
                    table.add_row(row);
                }

                print!("{}", table);
            }
        }
    };
}

pub(crate) use output;
