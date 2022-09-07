use playlist_sync::Result;
use playlist_sync::{qobuz, spotify};
use rspotify::model::PlaylistId;
use std::str::FromStr;

// QOBUZ PLAYLIST https://play.qobuz.com/playlist/3551270
// SPOTIFY PLAYLIST https://open.spotify.com/playlist/2IkvmS2LOZJCFa6n9yiA7Z

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let spotify = spotify::new().await;
    let qobuz = qobuz::new().await;

    let spotify_playlist = spotify
        .playlist(
            PlaylistId::from_str("2IkvmS2LOZJCFa6n9yiA7Z").expect("invalid spotify playlist id"),
        )
        .await?;

    let qobuz_playlist = qobuz.playlist(3551270.to_string()).await?;

    let qobuz_isrcs = qobuz_playlist.irsc_list();
    let missing_tracks = spotify_playlist.missing_tracks(qobuz_isrcs.clone());

    println!("spotify size: {}", spotify_playlist.track_count());
    println!("spotify isrc: {}", spotify_playlist.isrc_list().len());
    println!("qobuz size: {}", qobuz_playlist.track_count());
    println!("qobuz isrc: {}", qobuz_isrcs.len());
    println!("missing tracks: {}", missing_tracks.len());

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
