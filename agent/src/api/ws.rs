use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::state::{AppState, WsSubscription};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "subscribe")]
    Subscribe { target: String, id: Option<i64> },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { target: String, id: Option<i64> },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    info!("WebSocket client connected");

    // Subscribe to global events by default
    state.ws_connections.subscribe(WsSubscription::Global, tx.clone()).await;

    // Task to receive messages from the channel and send to client
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender
                .send(axum::extract::ws::Message::Text(msg.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Task to receive messages from client
    let tx_clone = tx.clone();
    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let axum::extract::ws::Message::Text(text) = msg {
                if let Err(e) = handle_client_message(&text, &state_clone, &tx_clone).await {
                    warn!("Error handling client message: {}", e);
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => {
            recv_task.abort();
        }
        _ = &mut recv_task => {
            send_task.abort();
        }
    }

    info!("WebSocket client disconnected");
}

async fn handle_client_message(
    text: &str,
    state: &AppState,
    tx: &mpsc::UnboundedSender<String>,
) -> Result<(), String> {
    let msg: ClientMessage = serde_json::from_str(text)
        .map_err(|e| format!("Failed to parse message: {}", e))?;

    match msg {
        ClientMessage::Subscribe { target, id } => {
            let subscription = match target.as_str() {
                "build" => {
                    let build_id = id.ok_or("Missing build_id")?;
                    WsSubscription::Build(build_id)
                }
                "project" => {
                    let project_id = id.ok_or("Missing project_id")?;
                    WsSubscription::Project(project_id)
                }
                "global" => WsSubscription::Global,
                _ => return Err(format!("Unknown target: {}", target)),
            };

            state.ws_connections.subscribe(subscription, tx.clone()).await;
            info!("Client subscribed to: {:?}", target);
        }
        ClientMessage::Unsubscribe { target, id } => {
            // Unsubscribe logic (simplified for now)
            info!("Client unsubscribed from: {:?}", target);
        }
    }

    Ok(())
}
