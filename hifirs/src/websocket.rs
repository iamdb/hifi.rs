use std::path::PathBuf;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use tower_http::services::ServeDir;

use crate::player::{self, controls::Action};

pub async fn init() {
    let assets_dir = PathBuf::from("www").join("build");
    let app = Router::new()
        .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        .route("/ws", get(handler));

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));

    debug!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_connection)
}

async fn handle_connection(socket: WebSocket) {
    debug!("new websocket connection");
    let (mut sender, mut receiver) = socket.split();

    let mut send_task = tokio::spawn(async move {
        debug!("spawning send task");
        let mut broadcast_receiver = player::notify_receiver();

        loop {
            if let Ok(message) = broadcast_receiver.recv().await {
                let json = serde_json::to_string(&message).expect("error making json");
                match sender.send(Message::Text(json)).await {
                    Ok(()) => {}
                    Err(error) => {
                        debug!(?error)
                    }
                }
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        debug!("spawning receive task");
        let controls = player::controls();

        while let Some(data) = receiver.next().await {
            match data {
                Ok(message) => {
                    if let Message::Text(s) = message {
                        if let Ok(action) = serde_json::from_str::<Action>(&s) {
                            debug!(?action);
                            match action {
                                Action::Play => controls.play().await,
                                Action::Pause => controls.pause().await,
                                Action::PlayPause => controls.play_pause().await,
                                Action::Next => controls.next().await,
                                Action::Previous => controls.previous().await,
                                Action::Stop => controls.stop().await,
                                Action::Quit => controls.quit().await,
                                Action::SkipTo { num } => controls.skip_to(num).await,
                                Action::SkipToById { track_id } => {
                                    controls.skip_to_by_id(track_id).await
                                }
                                Action::JumpForward => controls.jump_forward().await,
                                Action::JumpBackward => controls.jump_backward().await,
                                Action::PlayAlbum { album_id } => {
                                    controls.play_album(album_id).await
                                }
                                Action::PlayTrack { track_id } => {
                                    controls.play_track(track_id).await
                                }
                                Action::PlayUri { uri } => controls.play_uri(uri).await,
                                Action::PlayPlaylist { playlist_id } => {
                                    controls.play_playlist(playlist_id).await
                                }
                            }
                        };
                    }
                }
                Err(_) => todo!(),
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}
