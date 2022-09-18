use crate::{qobuz, spotify, Result};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rspotify::model::PlaylistId;
use snafu::Snafu;
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
    pretty_env_logger::init();
    let cli = Cli::parse();

    let prog = MultiProgress::new();

    let spotify_prog = ProgressBar::new_spinner();
    spotify_prog.set_style(ProgressStyle::with_template("[{bar:40.cyan/blue}]\n{msg}").unwrap());
    prog.add(spotify_prog.clone());

    let mut spotify = spotify::new(&spotify_prog).await;
    spotify.auth().await;

    let qobuz = qobuz::new().await;

    let spotify_playlist = spotify
        .playlist(
            PlaylistId::from_str(cli.spotify_playlist_id.as_str())
                .expect("invalid spotify playlist id"),
        )
        .await?;

    let qobuz_playlist = qobuz.playlist(cli.qobuz_playlist_id).await?;

    let qobuz_isrcs = qobuz_playlist.irsc_list();
    let missing_tracks = spotify_playlist.missing_tracks(qobuz_isrcs.clone());

    let progress = ProgressBar::new(missing_tracks.len() as u64);
    progress.set_style(ProgressStyle::with_template("[{bar:40.cyan/blue}]\n{msg}").unwrap());

    prog.add(progress.clone());

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
