use crate::{
    player,
    qobuz::{
        self,
        client::{self, output, Credentials, OutputFormat},
        search_results::SearchResults,
    },
    state::{
        self,
        app::{ClientKey, StateKey},
        AudioQuality, StringValue,
    },
    switch_screen, ui, wait, REFRESH_RESOLUTION,
};
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Table};
use dialoguer::{Confirm, Input, Password};
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
        url: String,
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
    Playlist { playlist_id: String },
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
        error: client::Error,
    },
    PlayerError {
        error: player::Error,
    },
    TerminalError {
        error: ui::Error,
    },
}

impl From<client::Error> for Error {
    fn from(error: client::Error) -> Self {
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
    let app_state = state::app::new(base_dir).expect("failed to setup database");

    let creds = Credentials {
        username: cli.username,
        password: cli.password,
    };

    // CLI COMMANDS
    match cli.command {
        Commands::Resume { no_tui } => {
            let client = client::new(app_state.clone(), creds).await?;
            let player = player::new(app_state.clone(), client.clone(), true).await;

            player.play();

            if no_tui {
                wait!(app_state);
            } else {
                let mut tui = ui::new(app_state, player.controls(), client, None, None)?;
                tui.event_loop().await?;
            }

            Ok(())
        }
        Commands::Play { url, no_tui } => {
            let client = client::new(app_state.clone(), creds).await?;
            let mut player = player::new(app_state.clone(), client.clone(), true).await;
            let mut quit = false;

            if let Some(url) = qobuz::parse_url(url.as_str()) {
                match url {
                    qobuz::UrlType::Album { id } => {
                        if let Ok(album) = client.album(id).await {
                            player.play_album(album, Some(client.quality())).await;
                        }
                    }
                    qobuz::UrlType::Playlist { id } => {
                        debug!("can't play a playlist yet, {}", id);
                        app_state.quit();
                        quit = true;
                    }
                }
            }

            if quit {
                Ok(())
            } else {
                if no_tui {
                    wait!(app_state);
                } else {
                    let mut tui = ui::new(app_state, player.controls(), client, None, None)?;
                    tui.event_loop().await?;
                }
                Ok(())
            }
        }
        #[allow(unused)]
        Commands::Search {
            query,
            limit,
            output_format,
        } => {
            let client = client::new(app_state.clone(), creds).await?;

            match client.search_all(query).await {
                Ok(results) => {
                    //let json = serde_json::to_string(&results);
                    print!("{}", results);
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
            let client = client::new(app_state.clone(), creds).await?;
            let results = SearchResults::Albums(client.search_albums(query.clone(), limit).await?);

            if no_tui {
                output!(results, output_format);
            } else {
                let player = player::new(app_state.clone(), client.clone(), true).await;

                if no_tui {
                    wait!(app_state);
                } else {
                    let mut tui = ui::new(
                        app_state,
                        player.controls(),
                        client,
                        Some(results),
                        Some(query),
                    )?;
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
            let client = client::new(app_state.clone(), creds).await?;
            let results =
                SearchResults::Artists(client.search_artists(query.clone(), limit).await?);

            if no_tui {
                output!(results, output_format);
            } else {
                let player = player::new(app_state.clone(), client.clone(), true).await;

                if no_tui {
                    wait!(app_state);
                } else {
                    let mut tui = ui::new(
                        app_state,
                        player.controls(),
                        client,
                        Some(results),
                        Some(query),
                    )?;
                    tui.event_loop().await?;
                }
            }

            Ok(())
        }
        #[allow(unused_variables)]
        Commands::MyPlaylists {
            no_tui,
            output_format,
        } => {
            let client = client::new(app_state.clone(), creds).await?;

            if no_tui {
                println!("nothing to show");
            } else {
                let player = player::new(app_state.clone(), client.clone(), true).await;

                if no_tui {
                    wait!(app_state);
                } else {
                    let mut tui =
                        ui::new(app_state.clone(), player.controls(), client, None, None)?;
                    switch_screen!(app_state, ActiveScreen::Playlists);
                    tui.event_loop().await?;
                }
            }

            Ok(())
        }
        Commands::Playlist { playlist_id } => {
            let client = client::new(app_state.clone(), creds).await?;
            let results = client.playlist(playlist_id).await?;
            let json =
                serde_json::to_string(&results).expect("failed to convert results to string");

            print!("{}", json);
            Ok(())
        }
        Commands::StreamTrack {
            track_id,
            quality,
            no_tui,
        } => {
            let client = client::new(app_state.clone(), creds).await?;
            let mut player = player::new(app_state.clone(), client.clone(), false).await;

            let track = client.track(track_id).await?;

            player.play_track(track, Some(quality.unwrap())).await;

            if no_tui {
                wait!(app_state);
            } else {
                let mut tui = ui::new(app_state, player.controls(), client, None, None)?;
                tui.event_loop().await?;
            }

            Ok(())
        }
        Commands::StreamAlbum {
            album_id,
            quality,
            no_tui,
        } => {
            let client = client::new(app_state.clone(), creds)
                .await
                .expect("failed to create client");

            let album = client.album(album_id).await?;

            let quality = if let Some(q) = quality {
                q
            } else {
                client.quality()
            };

            let mut player = player::new(app_state.clone(), client.clone(), false).await;
            player.play_album(album, Some(quality)).await;

            if no_tui {
                wait!(app_state);
            } else {
                let mut tui = ui::new(app_state.clone(), player.controls(), client, None, None)?;
                switch_screen!(app_state, ActiveScreen::NowPlaying);
                tui.event_loop().await?;
            }

            Ok(())
        }
        Commands::Reset => {
            app_state.player.clear();
            app_state.player.flush();
            Ok(())
        }
        Commands::Config { command } => match command {
            ConfigCommands::Username {} => {
                let username: String = Input::new()
                    .with_prompt("Enter your username / email")
                    .interact_text()
                    .expect("failed to get username");

                app_state.config.insert::<String, StringValue>(
                    StateKey::Client(ClientKey::Username),
                    username.into(),
                );

                println!("Username saved.");

                Ok(())
            }
            ConfigCommands::Password {} => {
                let password: String = Password::new()
                    .with_prompt("Enter your password (hidden)")
                    .interact()
                    .expect("failed to get password");

                let md5_pw = format!("{:x}", md5::compute(password));

                debug!("saving password to database: {}", md5_pw);

                app_state.config.insert::<String, StringValue>(
                    StateKey::Client(ClientKey::Password),
                    md5_pw.into(),
                );

                println!("Password saved.");

                Ok(())
            }
            ConfigCommands::DefaultQuality { quality } => {
                app_state.config.insert::<String, AudioQuality>(
                    StateKey::Client(ClientKey::DefaultQuality),
                    quality,
                );

                println!("Default quality saved.");

                Ok(())
            }
            ConfigCommands::Clear {} => {
                if Confirm::new()
                    .with_prompt("This will clear the configuration in the database.\nDo you want to continue?")
                    .interact()
                    .expect("failed to get response")
                {
                    app_state.config.clear();

                    println!("Database cleared.");

                    Ok(())
                } else {
                    Ok(())
                }
            }
        },
    }
}

#[macro_export]
macro_rules! wait {
    ($state:expr) => {
        let mut quitter = $state.quitter();

        let state = $state.clone();
        ctrlc::set_handler(move || {
            state.quit();
            std::process::exit(0);
        })
        .expect("error setting ctrlc handler");

        loop {
            if let Ok(quit) = quitter.try_recv() {
                if quit {
                    debug!("quitting");
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(REFRESH_RESOLUTION));
        }
    };
}
