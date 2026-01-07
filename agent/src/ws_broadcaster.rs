use anyhow::Result;
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::state::AppState;

pub async fn run_ws_broadcaster(state: AppState) -> Result<()> {
    info!("WebSocket broadcaster started");

    let mut event_rx = state.subscribe_events();

    loop {
        tokio::select! {
            Ok(event) = event_rx.recv() => {
                // Broadcast event to all connected WebSocket clients
                state.ws_connections.broadcast_event(&event).await;
            }
            _ = sleep(Duration::from_secs(1)) => {
                // Periodic cleanup or heartbeat if needed
            }
        }
    }
}
