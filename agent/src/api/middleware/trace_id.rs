use axum::{
    body::Body,
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::Response,
};
use tower::{Layer, Service};
use std::task::{Context, Poll};

use crate::infrastructure::logging::TraceContext;

/// Trace ID를 HTTP 요청에 추가하는 미들웨어
///
/// - x-trace-id 헤더가 있으면 사용
/// - 없으면 새로 생성
/// - 응답 헤더에도 trace_id 추가
pub async fn add_trace_id(mut request: Request, next: Next) -> Response {
    // Extract or generate trace ID
    let trace_id = TraceContext::extract_or_generate(request.headers());

    // Add trace_id to request extensions for handlers to access
    request.extensions_mut().insert(trace_id.clone());

    // Call the next middleware/handler
    let mut response = next.run(request).await;

    // Add trace_id to response headers
    if let Ok(header_value) = HeaderValue::from_str(&trace_id) {
        response.headers_mut().insert("x-trace-id", header_value);
    }

    response
}

/// TraceIdLayer - Axum Layer wrapper
#[derive(Clone)]
pub struct TraceIdLayer;

impl<S> Layer<S> for TraceIdLayer {
    type Service = TraceIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TraceIdService { inner }
    }
}

#[derive(Clone)]
pub struct TraceIdService<S> {
    inner: S,
}

impl<S> Service<Request> for TraceIdService<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Extract or generate trace ID
            let trace_id = TraceContext::extract_or_generate(req.headers());

            // Add to request extensions
            req.extensions_mut().insert(trace_id.clone());

            // Call inner service
            let mut response = inner.call(req).await?;

            // Add to response headers
            if let Ok(header_value) = HeaderValue::from_str(&trace_id) {
                response.headers_mut().insert("x-trace-id", header_value);
            }

            Ok(response)
        })
    }
}

/// Request extension helper to extract trace_id
pub trait TraceIdExt {
    fn trace_id(&self) -> String;
}

impl TraceIdExt for Request {
    fn trace_id(&self) -> String {
        self.extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(TraceContext::new_trace_id)
    }
}
