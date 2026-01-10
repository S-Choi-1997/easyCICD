use std::time::Instant;
use tracing::{info, warn, error};

/// BoundaryLogger - 모든 모듈 경계에서 로깅을 담당
///
/// 로깅 포맷: [시간] [레벨] [trace_id] [호출자→피호출자] [작업] [상태] [duration]
#[derive(Clone)]
pub struct BoundaryLogger;

impl BoundaryLogger {
    pub fn new() -> Self {
        Self
    }

    /// API 엔드포인트 호출 시작
    /// 예: [API→Handler] POST /api/projects [ENTRY] name="my-app"
    pub fn api_entry(&self, trace_id: &str, method: &str, path: &str, params: &str) {
        info!(
            trace_id = %trace_id,
            method = %method,
            path = %path,
            flow = "API→Handler",
            stage = "ENTRY",
            "[{}] [API→Handler] {} {} [ENTRY] {}",
            trace_id, method, path, params
        );
    }

    /// API 엔드포인트 완료
    /// 예: [API→Handler] POST /api/projects [←DONE] 150ms status=201
    pub fn api_exit(&self, trace_id: &str, method: &str, path: &str, duration_ms: f64, status: u16) {
        info!(
            trace_id = %trace_id,
            method = %method,
            path = %path,
            flow = "API→Handler",
            stage = "←DONE",
            duration_ms = %duration_ms,
            status = %status,
            "[{}] [API→Handler] {} {} [←DONE] {:.2}ms status={}",
            trace_id, method, path, duration_ms, status
        );
    }

    /// API 에러
    pub fn api_error<E: std::fmt::Display>(&self, trace_id: &str, method: &str, path: &str, duration_ms: f64, error: &E) {
        error!(
            trace_id = %trace_id,
            method = %method,
            path = %path,
            flow = "API→Handler",
            stage = "←FAIL",
            duration_ms = %duration_ms,
            error = %error,
            "[{}] [API→Handler] {} {} [←FAIL] {:.2}ms error={}",
            trace_id, method, path, duration_ms, error
        );
    }

    /// 서비스 레이어 호출 시작
    /// 예: [API→BuildService] execute_build [ENTRY] build_id=42
    pub fn service_entry<T: std::fmt::Debug>(&self, trace_id: &str, from: &str, service: &str, method: &str, params: &T) {
        info!(
            trace_id = %trace_id,
            from = %from,
            service = %service,
            method = %method,
            flow = format!("{}→{}", from, service),
            stage = "ENTRY",
            params = ?params,
            "[{}] [{}→{}] {} [ENTRY] params={:?}",
            trace_id, from, service, method, params
        );
    }

    /// 서비스 레이어 완료
    /// 예: [API→BuildService] execute_build [←DONE] 27500ms
    pub fn service_exit(&self, trace_id: &str, from: &str, service: &str, method: &str, duration_ms: f64) {
        info!(
            trace_id = %trace_id,
            from = %from,
            service = %service,
            method = %method,
            flow = format!("{}→{}", from, service),
            stage = "←DONE",
            duration_ms = %duration_ms,
            "[{}] [{}→{}] {} [←DONE] {:.2}ms",
            trace_id, from, service, method, duration_ms
        );
    }

    /// 서비스 레이어 에러
    pub fn service_error<E: std::fmt::Display>(&self, trace_id: &str, from: &str, service: &str, method: &str, error: &E) {
        error!(
            trace_id = %trace_id,
            from = %from,
            service = %service,
            method = %method,
            flow = format!("{}→{}", from, service),
            stage = "←FAIL",
            error = %error,
            "[{}] [{}→{}] {} [←FAIL] error={}",
            trace_id, from, service, method, error
        );
    }

    /// 레포지토리 호출 시작
    /// 예: [BuildService→ProjectRepo] get [CALL]
    pub fn repo_call(&self, trace_id: &str, from: &str, repo: &str, method: &str) {
        info!(
            trace_id = %trace_id,
            from = %from,
            repo = %repo,
            method = %method,
            flow = format!("{}→{}", from, repo),
            stage = "CALL",
            "[{}] [{}→{}] {} [CALL]",
            trace_id, from, repo, method
        );
    }

    /// 레포지토리 완료
    /// 예: [BuildService→ProjectRepo] get [←DONE] 5ms
    pub fn repo_done(&self, trace_id: &str, from: &str, repo: &str, method: &str, duration_ms: f64) {
        info!(
            trace_id = %trace_id,
            from = %from,
            repo = %repo,
            method = %method,
            flow = format!("{}→{}", from, repo),
            stage = "←DONE",
            duration_ms = %duration_ms,
            "[{}] [{}→{}] {} [←DONE] {:.2}ms",
            trace_id, from, repo, method, duration_ms
        );
    }

    /// 레포지토리 에러
    pub fn repo_error<E: std::fmt::Display>(&self, trace_id: &str, from: &str, repo: &str, method: &str, error: &E) {
        error!(
            trace_id = %trace_id,
            from = %from,
            repo = %repo,
            method = %method,
            flow = format!("{}→{}", from, repo),
            stage = "←FAIL",
            error = %error,
            "[{}] [{}→{}] {} [←FAIL] error={}",
            trace_id, from, repo, method, error
        );
    }

    /// 외부 시스템 호출 시작 (Docker, GitHub 등)
    /// 예: [BuildService→Docker] run_build [EXT→]
    pub fn external_call(&self, trace_id: &str, from: &str, system: &str, operation: &str) {
        info!(
            trace_id = %trace_id,
            from = %from,
            system = %system,
            operation = %operation,
            flow = format!("{}→{}", from, system),
            stage = "EXT→",
            "[{}] [{}→{}] {} [EXT→]",
            trace_id, from, system, operation
        );
    }

    /// 외부 시스템 완료
    /// 예: [BuildService→Docker] run_build [←DONE] 22000ms
    pub fn external_done(&self, trace_id: &str, from: &str, system: &str, operation: &str, duration_ms: f64) {
        info!(
            trace_id = %trace_id,
            from = %from,
            system = %system,
            operation = %operation,
            flow = format!("{}→{}", from, system),
            stage = "←DONE",
            duration_ms = %duration_ms,
            "[{}] [{}→{}] {} [←DONE] {:.2}ms",
            trace_id, from, system, operation, duration_ms
        );
    }

    /// 외부 시스템 에러
    pub fn external_error<E: std::fmt::Display>(&self, trace_id: &str, from: &str, system: &str, operation: &str, error: &E) {
        error!(
            trace_id = %trace_id,
            from = %from,
            system = %system,
            operation = %operation,
            flow = format!("{}→{}", from, system),
            stage = "←FAIL",
            error = %error,
            "[{}] [{}→{}] {} [←FAIL] error={}",
            trace_id, from, system, operation, error
        );
    }

    /// 이벤트 발행
    /// 예: [BuildService→EventBus] emit [EVT↗] BuildStatus::Building
    pub fn event_emit(&self, trace_id: &str, from: &str, event_type: &str) {
        info!(
            trace_id = %trace_id,
            from = %from,
            event_type = %event_type,
            flow = format!("{}→EventBus", from),
            stage = "EVT↗",
            "[{}] [{}→EventBus] emit [EVT↗] {}",
            trace_id, from, event_type
        );
    }
}

impl Default for BoundaryLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// 성능 측정용 타이머
pub struct Timer {
    start: Instant,
}

impl Timer {
    /// 타이머 시작
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// 경과 시간 (밀리초)
    pub fn elapsed_ms(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }
}
