use async_trait::async_trait;
use crate::events::Event;
use tokio::sync::broadcast;

/// Trait for event bus operations
#[async_trait]
pub trait EventBus: Send + Sync + Clone {
    /// Emit an event to all subscribers
    async fn emit(&self, event: Event);

    /// Subscribe to events
    /// Returns a receiver for listening to events
    fn subscribe(&self) -> broadcast::Receiver<Event>;
}
