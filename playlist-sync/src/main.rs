// QOBUZ PLAYLIST https://play.qobuz.com/playlist/3551270
// SPOTIFY PLAYLIST https://open.spotify.com/playlist/2IkvmS2LOZJCFa6n9yiA7Z

use playlist_sync::cli;

#[tokio::main]
async fn main() -> Result<(), cli::Error> {
    crate::cli::run().await
}
