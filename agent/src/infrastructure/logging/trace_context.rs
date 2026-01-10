use axum::http::HeaderMap;

/// Trace ID 생성 및 전파를 담당
pub struct TraceContext;

impl TraceContext {
    /// 새 Trace ID 생성 (UUID v4)
    pub fn new_trace_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// HTTP 헤더에서 Trace ID 추출 (없으면 생성)
    ///
    /// x-trace-id 헤더를 찾고, 없으면 새로 생성
    pub fn extract_or_generate(headers: &HeaderMap) -> String {
        headers
            .get("x-trace-id")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(Self::new_trace_id)
    }

    /// Trace ID를 HTTP 헤더에 추가
    pub fn add_to_headers(headers: &mut HeaderMap, trace_id: &str) {
        if let Ok(value) = trace_id.parse() {
            headers.insert("x-trace-id", value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_trace_id() {
        let id1 = TraceContext::new_trace_id();
        let id2 = TraceContext::new_trace_id();

        assert_ne!(id1, id2);
        assert!(id1.len() > 0);
    }

    #[test]
    fn test_extract_or_generate_with_existing() {
        let mut headers = HeaderMap::new();
        headers.insert("x-trace-id", "test-trace-id".parse().unwrap());

        let trace_id = TraceContext::extract_or_generate(&headers);
        assert_eq!(trace_id, "test-trace-id");
    }

    #[test]
    fn test_extract_or_generate_without_existing() {
        let headers = HeaderMap::new();
        let trace_id = TraceContext::extract_or_generate(&headers);

        assert!(trace_id.len() > 0);
        // Should be a valid UUID format
        assert!(trace_id.contains('-'));
    }
}
