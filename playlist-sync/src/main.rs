use playlist_sync::Result;
use playlist_sync::{qobuz, spotify};
use rspotify::model::PlaylistId;
use spinoff::{Color, Spinner, Spinners};
use std::str::FromStr;
use std::time::Duration;

// QOBUZ PLAYLIST https://play.qobuz.com/playlist/3551270
// SPOTIFY PLAYLIST https://open.spotify.com/playlist/2IkvmS2LOZJCFa6n9yiA7Z

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    println!(
        r#"
╔═╗ ┌─┐┌┐ ┬ ┬┌─┐      
║═╬╗│ │├┴┐│ │┌─┘      
╚═╝╚└─┘└─┘└─┘└─┘      
╔═╗┬  ┌─┐┬ ┬┬  ┬┌─┐┌┬┐
╠═╝│  ├─┤└┬┘│  │└─┐ │ 
╩  ┴─┘┴ ┴ ┴ ┴─┘┴└─┘ ┴ 
╔═╗┬ ┬┌┐┌┌─┐          
╚═╗└┬┘││││            
╚═╝ ┴ ┘└┘└─┘
"#
    );
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
            PlaylistId::from_str("2IkvmS2LOZJCFa6n9yiA7Z").expect("invalid spotify playlist id"),
        )
        .await?;
    spinner.success("Playlist retrieved from Spotify");

    let spinner = Spinner::new(Spinners::Dots, "Qobuz", Color::Blue);
    let qobuz_playlist = qobuz.playlist(3551270.to_string()).await?;
    spinner.success("Playlist retreived from Qobuz.");

    let spinner = Spinner::new(Spinners::Dots, "Analyzing...", Color::Blue);
    let qobuz_isrcs = qobuz_playlist.irsc_list();
    let missing_tracks = spotify_playlist.missing_tracks(qobuz_isrcs.clone());
    spinner.stop_with_message(&format!(
        "\n\nTotal Spotify Tracks: {}\nTotal Qobuz Tracks {}\n",
        spotify_playlist.track_count(),
        qobuz_playlist.track_count()
    ));

    println!("Searching for missing tracks");
    let mut spinner = Spinner::new(Spinners::Dots, "Searching...", Color::Blue);
    for missing in missing_tracks {
        if let Some(isrc) = missing.track.external_ids.get("isrc") {
            spinner.update_text(format!("Searching for track isrc: {}", isrc.to_lowercase()));
            let results = qobuz.search(isrc.to_lowercase()).await;
            if !results.is_empty() {
                if let Some(found) = results.get(0) {
                    println!(
                        "\nFound track on Qobuz, adding to playlist: {}",
                        found.title
                    );

                    qobuz
                        .add_track(found.id.to_string(), qobuz_playlist.id())
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
                spinner.update_text(format!(
                    "Spotify track isrc not found: {}",
                    isrc.to_lowercase()
                ));
            }

            std::thread::sleep(Duration::from_millis(125));
        }
    }

    Ok(())
}

// pub async fn sync_playlists(
//     spotify_id: PlaylistId,
//     qobuz_id: String,
//     spotify_client: AuthCodeSpotify,
//     qobuz_client: Client,
// ) {
//
// }
