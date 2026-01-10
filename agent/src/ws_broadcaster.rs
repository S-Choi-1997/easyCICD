use anyhow::Result;
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::state::AppContext;

pub async fn run_ws_broadcaster(context: AppContext) -> Result<()> {
    info!("WebSocket broadcaster started");

    let mut event_rx = context.event_bus.subscribe();

    loop {
        tokio::select! {
            Ok(event) = event_rx.recv() => {
                // Broadcast event to all connected WebSocket clients
                context.ws_connections.broadcast_event(&event).await;
            }
            _ = sleep(Duration::from_secs(1)) => {
                // Periodic cleanup or heartbeat if needed
            }
        }
    }
}
