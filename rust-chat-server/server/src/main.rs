use std::sync::Arc;

use anyhow::Context;
use room_manager::RoomManagerBuilder;
use tokio::{net::TcpListener, signal::ctrl_c, sync::broadcast, task::JoinSet};

use crate::room_manager::ChatRoomMetadata;

mod room_manager;
mod session;

const PORT: u16 = 8080;
const CHAT_ROOMS_METADATA: &str = include_str!("../resources/chat_rooms_metadata.json");

#[tokio::main]
async fn main() {
    let chat_room_metadata: Vec<ChatRoomMetadata> = serde_json::from_str(CHAT_ROOMS_METADATA)
        .expect("could not parse the chat rooms metadatas");
    let room_manager = Arc::new(
        chat_room_metadata
            .into_iter()
            .fold(RoomManagerBuilder::new(), |builder, metadata| {
                builder.create_room(metadata)
            })
            .build(),
    );

    let mut join_set: JoinSet<anyhow::Result<()>> = JoinSet::new();
    let server = TcpListener::bind(format!("0.0.0.0:{}", PORT))
        .await
        .expect("could not bind to the port");
    let (quit_tx, quit_rx) = broadcast::channel::<()>(1);

    println!("Listening on port {}", PORT);
    loop {
        tokio::select! {
            Ok(_) = ctrl_c() => {
                println!("Server interrupted. Gracefully shutting down.");
                quit_tx.send(()).context("failed to send quit signal").unwrap();
                break;
            }
            Ok((socket, _)) = server.accept() => {
                join_set.spawn(session::handle_user_session(Arc::clone(&room_manager), quit_rx.resubscribe(), socket));
            }
        }
    }

    while join_set.join_next().await.is_some() {}
    println!("Server shut down");
}
