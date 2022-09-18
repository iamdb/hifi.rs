use crate::{qobuz, spotify, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rspotify::model::PlaylistId;
use snafu::Snafu;
use spinoff::{Color, Spinner, Spinners};
use std::{str::FromStr, time::Duration};

const TITLE: &str = r#"
╔═╗ ┌─┐┌┐ ┬ ┬┌─┐      
║═╬╗│ │├┴┐│ │┌─┘      
╚═╝╚└─┘└─┘└─┘└─┘      
╔═╗┬  ┌─┐┬ ┬┬  ┬┌─┐┌┬┐
╠═╝│  ├─┤└┬┘│  │└─┐ │ 
╩  ┴─┘┴ ┴ ┴ ┴─┘┴└─┘ ┴ 
╔═╗┬ ┬┌┐┌┌─┐          
╚═╗└┬┘││││            
╚═╝ ┴ ┘└┘└─┘
"#;

#[derive(Parser)]
#[clap(name = TITLE, about = "spotify to qobuz one-way sync", long_about = None, rename_all = "camelCase")]
struct Cli {
    /// Spotify playlist to sync from
    #[clap(short = 's', long = "spotify")]
    pub spotify_playlist_id: String,
    /// Qobuz client to sync to
    #[clap(short = 'q', long = "qobuz")]
    pub qobuz_playlist_id: String,
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Client Error: {error}"))]
    QobuzError { error: qobuz::Error },
    #[snafu(display("Client Error: {error}"))]
    SpotifyError { error: spotify::Error },
}

impl From<spotify::Error> for Error {
    fn from(error: spotify::Error) -> Self {
        Error::SpotifyError { error }
    }
}

impl From<qobuz::Error> for Error {
    fn from(error: qobuz::Error) -> Self {
        Error::QobuzError { error }
    }
}

pub async fn run() -> Result<(), Error> {
    let cli = Cli::parse();

    pretty_env_logger::init();

    println!("spotify ⟶ qobuz one-way sync\n");
    std::thread::sleep(Duration::from_millis(500));

    println!("█▓▒▒░░░ Building api clients ░░░▒▒▓█\n");
    let spinner = Spinner::new(Spinners::Dots, "Building Spotify client...", Color::Green);
    let mut spotify = spotify::new().await;
    spotify.auth().await;
    spinner.success("Spotify client built.");

    let spinner = Spinner::new(Spinners::Dots, "Building Qobuz client...", Color::Green);
    let qobuz = qobuz::new().await;
    spinner.success("Qobuz client built.");

    println!("\n\n█▓▒▒░░░ Synchronizing playlists ░░░▒▒▓█\n");

    println!("Fetching playlists");
    let spinner = Spinner::new(Spinners::Dots, "Spotify", Color::Green);
    let spotify_playlist = spotify
        .playlist(
            PlaylistId::from_str(cli.spotify_playlist_id.as_str())
                .expect("invalid spotify playlist id"),
        )
        .await?;
    spinner.success("Playlist retrieved from Spotify");

    let spinner = Spinner::new(Spinners::Dots, "Qobuz", Color::Blue);
    let qobuz_playlist = qobuz.playlist(cli.qobuz_playlist_id).await?;
    spinner.success("Playlist retreived from Qobuz.");

    let spinner = Spinner::new(Spinners::Dots, "Analyzing...", Color::Blue);

    let qobuz_isrcs = qobuz_playlist.irsc_list();
    let missing_tracks = spotify_playlist.missing_tracks(qobuz_isrcs.clone());

    spinner.stop_with_message(&format!(
        "\nTotal Spotify Tracks: {}\nTotal Qobuz Tracks {}\nMissing Tracks {}\n",
        spotify_playlist.track_count(),
        qobuz_playlist.track_count(),
        missing_tracks.len()
    ));

    println!("Searching for missing tracks");
    let progress = ProgressBar::new(missing_tracks.len() as u64);
    progress.set_style(ProgressStyle::with_template("[{bar:40.cyan/blue}]\n{msg}").unwrap());

    for missing in missing_tracks {
        if let Some(isrc) = missing.track.external_ids.get("isrc") {
            progress.set_message(format!("Searching for track isrc: {}", isrc.to_lowercase()));
            let results = qobuz.search(isrc.to_lowercase()).await;
            if !results.is_empty() {
                if let Some(found) = results.get(0) {
                    qobuz
                        .add_track(qobuz_playlist.id(), found.id.to_string())
                        .await;

                    if missing.index < qobuz_playlist.track_count() {
                        qobuz
                            .update_track_position(
                                qobuz_playlist.id(),
                                found.id.to_string(),
                                missing.index - 1,
                            )
                            .await;
                    }
                }
            } else {
                progress.set_message(format!(
                    "Spotify track isrc not found: {}",
                    isrc.to_lowercase()
                ));
            }

            std::thread::sleep(Duration::from_millis(125));
        }

        progress.inc(1);
    }

    progress.finish();

    Ok(())
}
