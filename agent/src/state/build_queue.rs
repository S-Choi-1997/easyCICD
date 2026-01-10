use std::collections::HashMap;
use tokio::sync::RwLock;

/// BuildQueue - 프로젝트별 빌드 큐 관리
///
/// 책임:
/// - 프로젝트별로 빌드를 큐잉
/// - 동일 프로젝트는 순차 실행, 다른 프로젝트는 병렬 실행
/// - 현재 처리 중인 빌드 추적
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
