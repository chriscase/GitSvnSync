//! WebSocket endpoint for live status updates.
//!
//! Clients connect to `/ws` and receive JSON messages whenever the sync state
//! changes, a new conflict is detected, or a webhook is received.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tokio::sync::broadcast;
use tracing::{debug, warn};

use crate::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/ws", get(ws_handler))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rx = state.ws_broadcast.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, rx))
}

async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    debug!("WebSocket client connected");

    // Send a welcome message
    let welcome = serde_json::json!({
        "type": "connected",
        "message": "GitSvnSync live updates",
    });
    if let Err(e) = socket.send(Message::Text(welcome.to_string())).await {
        warn!("failed to send welcome message: {}", e);
        return;
    }

    // Forward broadcast messages to the WebSocket client.
    // If the client sends a close frame or the broadcast channel is closed, we exit.
    loop {
        tokio::select! {
            // Receive broadcast messages and forward to client
            result = rx.recv() => {
                match result {
                    Ok(msg) => {
                        if let Err(e) = socket.send(Message::Text(msg)).await {
                            debug!("WebSocket send error (client disconnected?): {}", e);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("WebSocket client lagged by {} messages", n);
                        // Send a lag notification
                        let lag_msg = serde_json::json!({
                            "type": "warning",
                            "message": format!("lagged by {} messages", n),
                        });
                        let _ = socket.send(Message::Text(lag_msg.to_string())).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        debug!("broadcast channel closed, disconnecting WebSocket");
                        break;
                    }
                }
            }
            // Listen for incoming messages from the client (mainly close frames)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => {
                        debug!("WebSocket client disconnected");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if let Err(e) = socket.send(Message::Pong(data)).await {
                            debug!("WebSocket pong error: {}", e);
                            break;
                        }
                    }
                    Some(Ok(_)) => {
                        // Ignore other messages from the client
                    }
                    Some(Err(e)) => {
                        debug!("WebSocket receive error: {}", e);
                        break;
                    }
                }
            }
        }
    }

    debug!("WebSocket connection closed");
}
