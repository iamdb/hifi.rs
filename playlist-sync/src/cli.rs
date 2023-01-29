use crate::{qobuz, spotify, Result};
use clap::Parser;
use console::Term;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use rspotify::model::PlaylistId;
use snafu::Snafu;
use std::time::Duration;

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
    pub qobuz_playlist_id: i64,
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

    let term = Term::stdout();
    let draw_target = ProgressDrawTarget::term(term.clone(), 15);
    let prog = MultiProgress::with_draw_target(draw_target);

    term.clear_screen().unwrap();

    println!("{TITLE}");

    let spotify_prog = ProgressBar::new_spinner().with_prefix("spotify");
    spotify_prog.enable_steady_tick(Duration::from_secs(1));
    spotify_prog.set_style(
        ProgressStyle::default_spinner()
            .template("{prefix} {spinner} {wide_msg}")
            .unwrap(),
    );

    prog.add(spotify_prog.clone());

    let mut spotify = spotify::new(&spotify_prog).await;
    spotify.auth().await;

    let qobuz_prog = ProgressBar::new_spinner().with_prefix("qobuz  ");
    qobuz_prog.enable_steady_tick(Duration::from_secs(1));
    qobuz_prog.set_style(
        ProgressStyle::default_spinner()
            .template("{prefix} {spinner} {wide_msg}")
            .unwrap(),
    );

    prog.add(qobuz_prog.clone());

    let mut qobuz = qobuz::new(&qobuz_prog).await;
    qobuz.auth().await;

    let spotify_playlist = spotify
        .playlist(
            PlaylistId::from_id(cli.spotify_playlist_id.as_str())
                .expect("invalid spotify playlist id"),
        )
        .await?;

    let qobuz_playlist = qobuz.playlist(cli.qobuz_playlist_id).await?;

    let qobuz_isrcs = qobuz_playlist.irsc_list();
    let missing_tracks = spotify_playlist.missing_tracks(qobuz_isrcs.clone());

    let progress = ProgressBar::new(missing_tracks.len() as u64).with_prefix("syncing");
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{prefix} {wide_bar:.cyan/blue} [{pos}/{len}]")
            .unwrap(),
    );

    prog.add(progress.clone());

    spotify_prog.finish_and_clear();

    for missing in missing_tracks {
        if let Some(isrc) = missing.track.external_ids.get("isrc") {
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
            }
            std::thread::sleep(Duration::from_millis(125));
        }

        progress.inc(1);
    }

    progress.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());
    progress.finish_with_message("complete!");
    Ok(())
}
