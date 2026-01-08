use anyhow::Result;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::db::models::Slot;
use crate::state::AppState;

pub async fn run_reverse_proxy(state: AppState) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await?;

    info!("Reverse proxy listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = state.clone();

        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        let state = state.clone();
                        async move { handle_request(req, state).await }
                    }),
                )
                .await
            {
                warn!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn handle_request(
    mut req: Request<Incoming>,
    state: AppState,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let path = req.uri().path();
    let headers = req.headers().clone();

    // Parse project name from path (/{project_name}/...)
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if parts.is_empty() || parts[0].is_empty() {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("Not found")))
            .unwrap());
    }

    let project_name = parts[0];

    // Get project from database
    let project = match state.db.get_project_by_name(project_name).await {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to get project {}: {}", project_name, e);
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from("Project not found")))
                .unwrap());
        }
    };

    // Determine target port based on active slot
    let target_port = match project.active_slot {
        Slot::Blue => project.blue_port,
        Slot::Green => project.green_port,
    };

    // Remove project name from path
    let target_path = if parts.len() > 1 {
        format!("/{}", parts[1..].join("/"))
    } else {
        "/".to_string()
    };

    // Preserve query string
    let target_uri = if let Some(query) = req.uri().query() {
        format!("http://{}:{}{}?{}", state.gateway_ip, target_port, target_path, query)
    } else {
        format!("http://{}:{}{}", state.gateway_ip, target_port, target_path)
    };

    info!(
        "Proxying {} {} -> {}",
        req.method(),
        path,
        target_uri
    );

    // Forward request to target
    let client = reqwest::Client::new();

    let method = match *req.method() {
        Method::GET => reqwest::Method::GET,
        Method::POST => reqwest::Method::POST,
        Method::PUT => reqwest::Method::PUT,
        Method::DELETE => reqwest::Method::DELETE,
        Method::PATCH => reqwest::Method::PATCH,
        Method::HEAD => reqwest::Method::HEAD,
        Method::OPTIONS => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    };

    // Collect request body
    let body_bytes = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            warn!("Failed to collect request body: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Bad request")))
                .unwrap());
        }
    };

    // Build request with headers
    let mut req_builder = client.request(method, &target_uri);

    // Copy headers except Host
    for (name, value) in headers.iter() {
        if name != "host" && name != "content-length" {
            if let Ok(value_str) = value.to_str() {
                req_builder = req_builder.header(name.as_str(), value_str);
            }
        }
    }

    let response = match req_builder
        .body(body_bytes.to_vec())
        .send()
        .await
    {
        Ok(res) => res,
        Err(e) => {
            warn!("Failed to proxy request: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from("Service unavailable")))
                .unwrap());
        }
    };

    // Convert response
    let status = response.status();
    let body = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            warn!("Failed to read response body: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from("Error reading response")))
                .unwrap());
        }
    };

    Ok(Response::builder()
        .status(status.as_u16())
        .body(Full::new(body))
        .unwrap())
}
