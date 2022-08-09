extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use crate::{
    cli::{Commands, ConfigCommands},
    qobuz::{
        client::{self, output, OutputFormat},
        PlaylistTrack,
    },
    state::{
        app::{AppState, ClientKey, PlayerKey, StateKey},
        AudioQuality, PlaylistValue, StringValue,
    },
};
use comfy_table::{presets::UTF8_FULL, Table};
use dialoguer::{console::Term, theme::ColorfulTheme, Confirm, Input, Password, Select};
use snafu::prelude::*;

pub mod cli;
mod mpris;
mod player;
pub mod qobuz;
pub mod state;
mod ui;

#[derive(Clone, Debug)]
pub struct Credentials {
    pub username: Option<String>,
    pub password: Option<String>,
}

pub const REFRESH_RESOLUTION: u64 = 250;

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
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
        error: ui::terminal::Error,
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

impl From<ui::terminal::Error> for Error {
    fn from(error: ui::terminal::Error) -> Self {
        Error::TerminalError { error }
    }
}

pub async fn cli(command: Commands, app_state: AppState, creds: Credentials) -> Result<(), Error> {
    pretty_env_logger::init();

    // CLI COMMANDS
    #[allow(unused)]
    match command {
        Commands::Resume { no_tui } => {
            let tree = app_state.player.clone();
            if let (Some(playlist), Some(next_up)) = (
                get_player!(PlayerKey::Playlist, tree, PlaylistValue),
                get_player!(PlayerKey::NextUp, tree, PlaylistTrack),
            ) {
                let client = client::new(app_state.clone(), creds).await?;

                let mut player = player::new(app_state.clone(), client.clone());
                player.setup(true).await;

                let next_track = player.attach_track_url(next_up).await?;

                if let Some(track_url) = next_track.track_url {
                    player.set_playlist(playlist);
                    player.set_uri(track_url);

                    if let Some(prev_playlist) =
                        get_player!(PlayerKey::PreviousPlaylist, tree, PlaylistValue)
                    {
                        player.set_prev_playlist(prev_playlist);
                    }

                    let controls = player.controls();
                    controls.play().await;

                    let mut tui = ui::terminal::new(app_state, controls, no_tui)?;
                    tui.start(client).await?;
                }
            } else {
                return Err(Error::PlayerError {
                    error: player::Error::Session,
                });
            }

            Ok(())
        }
        Commands::Play { query, quality } => {
            let client = client::new(app_state.clone(), creds).await?;

            let mut results = client.search_albums(query, Some(100)).await?;

            let album_list = results
                .albums
                .items
                .iter()
                .map(|i| {
                    format!(
                        "{} - {} ({})",
                        i.title,
                        i.artist.name,
                        i.release_date_original.get(0..4).unwrap()
                    )
                })
                .collect::<Vec<String>>();

            let selected = Select::with_theme(&ColorfulTheme::default())
                .items(&album_list)
                .default(0)
                .max_length(10)
                .interact_on_opt(&Term::stderr())
                .expect("There was a problem saving your selection.");

            if let Some(index) = selected {
                let selected_album = results.albums.items.remove(index);

                app_state.player.clear();

                let quality = if let Some(q) = quality {
                    q
                } else {
                    client.quality()
                };

                let player = player::new(app_state.clone(), client.clone());
                player.setup(false).await;

                if let Ok(album) = client.album(selected_album.id).await {
                    player.play_album(album, quality).await;

                    let mut tui = ui::terminal::new(app_state, player.controls(), false)?;
                    tui.start(client).await?;
                }
            }

            Ok(())
        }
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
        } => {
            let client = client::new(app_state.clone(), creds).await?;
            let results = client.search_albums(query, limit).await?;

            output!(results, output_format);
            Ok(())
        }
        Commands::GetAlbum { id, output_format } => {
            let client = client::new(app_state.clone(), creds).await?;
            let results = client.album(id).await?;

            output!(results, output_format);
            Ok(())
        }
        Commands::SearchArtists {
            query,
            limit,
            output_format,
        } => {
            let client = client::new(app_state.clone(), creds).await?;
            let results = client.search_artists(query, limit).await?;

            output!(results, output_format);
            Ok(())
        }
        Commands::GetArtist { id, output_format } => {
            let client = client::new(app_state.clone(), creds).await?;
            let results = client.artist(id, None).await?;

            output!(results, output_format);
            Ok(())
        }
        Commands::GetTrack { id, output_format } => {
            let client = client::new(app_state.clone(), creds).await?;
            let results = client.track(id).await?;
            let json =
                serde_json::to_string(&results).expect("failed to convert results to string");

            print!("{}", json);
            Ok(())
        }
        Commands::MyPlaylists {} => {
            let client = client::new(app_state.clone(), creds).await?;
            let results = client.user_playlists().await?;
            let json =
                serde_json::to_string(&results).expect("failed to convert results to string");

            print!("{}", json);
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
        Commands::StreamTrack { track_id, quality } => {
            let client = client::new(app_state.clone(), creds).await?;
            let player = player::new(app_state.clone(), client.clone());
            let track = client.track(track_id).await?;

            app_state.player.clear();
            player.setup(false).await;
            player.play_track(track, quality.unwrap()).await;

            let mut tui = ui::terminal::new(app_state, player.controls(), false)?;
            tui.start(client).await?;

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

            let player = player::new(app_state.clone(), client.clone());
            let album = client.album(album_id).await?;

            app_state.player.clear();
            player.setup(false).await;

            let quality = if let Some(q) = quality {
                q
            } else {
                client.quality()
            };

            player.play_album(album, quality).await;

            let mut tui = ui::terminal::new(app_state, player.controls(), no_tui)?;
            tui.start(client).await?;

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
