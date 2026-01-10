use async_trait::async_trait;
use tokio::sync::broadcast;
use std::sync::Arc;

use super::event_bus::EventBus;
use crate::events::Event;
use crate::infrastructure::logging::BoundaryLogger;

/// Broadcast-based implementation of EventBus
#[derive(Clone)]
pub struct BroadcastEventBus {
    tx: broadcast::Sender<Event>,
    logger: Arc<BoundaryLogger>,
}

impl BroadcastEventBus {
    /// Create a new BroadcastEventBus with given capacity
    pub fn new(capacity: usize, logger: Arc<BoundaryLogger>) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx, logger }
    }

    /// Create with default capacity (1000 events)
    pub fn new_default(logger: Arc<BoundaryLogger>) -> Self {
        Self::new(1000, logger)
    }
}

#[async_trait]
impl EventBus for BroadcastEventBus {
    async fn emit(&self, event: Event) {
        // Log event emission
        let event_type = match &event {
            Event::BuildStatus { .. } => "BuildStatus",
            Event::Log { .. } => "Log",
            Event::Deployment { .. } => "Deployment",
            Event::HealthCheck { .. } => "HealthCheck",
            Event::ContainerStatus { .. } => "ContainerStatus",
            Event::Error { .. } => "Error",
        };

        // We don't have trace_id in Event, so we use a generic marker
        self.logger.event_emit("system", "EventBus", event_type);

        // Emit event (ignore if no receivers)
        let _ = self.tx.send(event);
    }

    fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::BuildStatus;

    #[tokio::test]
    async fn test_event_bus_emit_and_subscribe() {
        let logger = Arc::new(BoundaryLogger::new());
        let bus = BroadcastEventBus::new_default(logger);

        let mut rx = bus.subscribe();

        let event = Event::build_status(1, 1, BuildStatus::Building);
        bus.emit(event.clone()).await;

        let received = rx.recv().await.unwrap();
        match (&event, &received) {
            (Event::BuildStatus { build_id: id1, .. }, Event::BuildStatus { build_id: id2, .. }) => {
                assert_eq!(id1, id2);
            }
            _ => panic!("Event type mismatch"),
        }
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let logger = Arc::new(BoundaryLogger::new());
        let bus = BroadcastEventBus::new_default(logger);

        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        let event = Event::build_status(2, 2, BuildStatus::Success);
        bus.emit(event).await;

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }
}
