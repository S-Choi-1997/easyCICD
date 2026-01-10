use std::sync::Arc;
use tokio::sync::broadcast;
use crate::db::Database;
use crate::events::Event;

// Re-export from state module
pub use crate::state::{BuildQueue, WsConnections, WsSubscription};

pub type EventSender = broadcast::Sender<Event>;
pub type EventReceiver = broadcast::Receiver<Event>;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub event_bus: EventSender,
    pub build_queue: Arc<BuildQueue>,
    pub ws_connections: Arc<WsConnections>,
    pub gateway_ip: String,
    pub base_domain: Option<String>,
}

impl AppState {
    pub fn new(db: Database, gateway_ip: String, base_domain: Option<String>) -> Self {
        let (event_tx, _) = broadcast::channel(100);

        Self {
            db,
            event_bus: event_tx,
            build_queue: Arc::new(BuildQueue::new()),
            ws_connections: Arc::new(WsConnections::new()),
            gateway_ip,
            base_domain,
        }
    }

    pub fn subscribe_events(&self) -> EventReceiver {
        self.event_bus.subscribe()
    }

    pub fn emit_event(&self, event: Event) {
        let _ = self.event_bus.send(event);
    }
}
