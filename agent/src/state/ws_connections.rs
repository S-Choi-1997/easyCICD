use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc};
use crate::events::Event;

/// WebSocket 구독 타입
#[derive(Clone)]
pub enum WsSubscription {
    Build(i64),
    Project(i64),
    Global,
}

/// WsConnections - WebSocket 연결 관리
///
/// 책임:
/// - WebSocket 클라이언트 연결 추적
/// - 구독 타입별 메시지 브로드캐스트
/// - 끊긴 연결 자동 정리
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
