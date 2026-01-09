use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, mpsc};
use crate::db::Database;
use crate::events::Event;

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

// Build Queue Manager
pub struct BuildQueue {
    // project_id -> queue of build_ids
    queues: RwLock<HashMap<i64, Vec<i64>>>,
    // Currently processing builds per project
    processing: RwLock<HashMap<i64, i64>>,
}

impl BuildQueue {
    pub fn new() -> Self {
        Self {
            queues: RwLock::new(HashMap::new()),
            processing: RwLock::new(HashMap::new()),
        }
    }

    pub async fn enqueue(&self, project_id: i64, build_id: i64) {
        let mut queues = self.queues.write().await;
        queues.entry(project_id).or_insert_with(Vec::new).push(build_id);
    }

    pub async fn dequeue(&self, project_id: i64) -> Option<i64> {
        let mut queues = self.queues.write().await;
        if let Some(queue) = queues.get_mut(&project_id) {
            if !queue.is_empty() {
                return Some(queue.remove(0));
            }
        }
        None
    }

    pub async fn is_processing(&self, project_id: i64) -> bool {
        let processing = self.processing.read().await;
        processing.contains_key(&project_id)
    }

    pub async fn start_processing(&self, project_id: i64, build_id: i64) {
        let mut processing = self.processing.write().await;
        processing.insert(project_id, build_id);
    }

    pub async fn finish_processing(&self, project_id: i64) {
        let mut processing = self.processing.write().await;
        processing.remove(&project_id);
    }

    pub async fn get_queue_length(&self, project_id: i64) -> usize {
        let queues = self.queues.read().await;
        queues.get(&project_id).map(|q| q.len()).unwrap_or(0)
    }

    pub async fn get_all_queued_builds(&self) -> Vec<(i64, Vec<i64>)> {
        let queues = self.queues.read().await;
        queues.iter().map(|(k, v)| (*k, v.clone())).collect()
    }
}

// WebSocket Connection Manager
#[derive(Clone)]
pub enum WsSubscription {
    Build(i64),
    Project(i64),
    Global,
}

pub struct WsConnections {
    // Subscription -> list of senders
    connections: RwLock<HashMap<String, Vec<mpsc::UnboundedSender<String>>>>,
}

impl WsConnections {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }

    fn subscription_key(sub: &WsSubscription) -> String {
        match sub {
            WsSubscription::Build(id) => format!("build:{}", id),
            WsSubscription::Project(id) => format!("project:{}", id),
            WsSubscription::Global => "global".to_string(),
        }
    }

    pub async fn subscribe(&self, sub: WsSubscription, tx: mpsc::UnboundedSender<String>) {
        let key = Self::subscription_key(&sub);
        let mut connections = self.connections.write().await;
        connections.entry(key).or_insert_with(Vec::new).push(tx);
    }

    pub async fn unsubscribe(&self, sub: WsSubscription, tx_id: usize) {
        let key = Self::subscription_key(&sub);
        let mut connections = self.connections.write().await;
        if let Some(senders) = connections.get_mut(&key) {
            if tx_id < senders.len() {
                senders.remove(tx_id);
            }
        }
    }

    pub async fn broadcast(&self, sub: WsSubscription, message: String) {
        let key = Self::subscription_key(&sub);
        let mut connections = self.connections.write().await;

        if let Some(senders) = connections.get_mut(&key) {
            // Remove closed connections and send to active ones
            senders.retain(|tx| tx.send(message.clone()).is_ok());
        }
    }

    pub async fn broadcast_event(&self, event: &Event) {
        let message = serde_json::to_string(event).unwrap_or_default();

        // Always broadcast to global
        self.broadcast(WsSubscription::Global, message.clone()).await;

        // Also broadcast to specific subscriptions based on event type
        match event {
            Event::BuildStatus { build_id, project_id, .. } => {
                self.broadcast(WsSubscription::Build(*build_id), message.clone()).await;
                self.broadcast(WsSubscription::Project(*project_id), message).await;
            },
            Event::Log { build_id, .. } => {
                self.broadcast(WsSubscription::Build(*build_id), message).await;
            },
            Event::Deployment { project_id, build_id, .. } => {
                self.broadcast(WsSubscription::Build(*build_id), message.clone()).await;
                self.broadcast(WsSubscription::Project(*project_id), message).await;
            },
            Event::HealthCheck { project_id, build_id, .. } => {
                self.broadcast(WsSubscription::Build(*build_id), message.clone()).await;
                self.broadcast(WsSubscription::Project(*project_id), message).await;
            },
            Event::ContainerStatus { project_id, .. } => {
                self.broadcast(WsSubscription::Project(*project_id), message).await;
            },
            Event::Error { project_id, build_id, .. } => {
                if let Some(bid) = build_id {
                    self.broadcast(WsSubscription::Build(*bid), message.clone()).await;
                }
                if let Some(pid) = project_id {
                    self.broadcast(WsSubscription::Project(*pid), message).await;
                }
            },
        }
    }

    pub async fn connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.values().map(|v| v.len()).sum()
    }
}
