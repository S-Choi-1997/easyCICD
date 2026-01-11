use anyhow::Result;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error};

use crate::state::AppContext;
use crate::application::events::event_bus::EventBus;

pub async fn run_ws_broadcaster(context: AppContext) -> Result<()> {
    info!("WebSocket broadcaster started");

    let mut event_rx = context.event_bus.subscribe();

    loop {
        tokio::select! {
            event_result = event_rx.recv() => {
                match event_result {
                    Ok(event) => {
                        // Broadcast event to all connected WebSocket clients
                        context.ws_connections.broadcast_event(&event).await;
                    }
                    Err(e) => {
                        error!("Event broadcast channel error: {:?}", e);
                        warn!("All event senders dropped, WebSocket broadcaster stopping");
                        return Err(anyhow::anyhow!("Event channel closed: {:?}", e));
                    }
                }
            }
            _ = sleep(Duration::from_secs(1)) => {
                // Periodic cleanup or heartbeat if needed
            }
        }
    }
}
