use playlist_sync::{qobuz, spotify};

#[tokio::main]
async fn main() {
    let spotify = spotify::new();
    let qobuz = qobuz::new();
}

// pub async fn sync_playlists(
//     spotify_id: PlaylistId,
//     qobuz_id: String,
//     spotify_client: AuthCodeSpotify,
//     qobuz_client: Client,
// ) {
//
// }
