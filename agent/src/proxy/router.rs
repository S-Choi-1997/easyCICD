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
    let method = req.method().clone();
    let headers = req.headers().clone();

    // Log all incoming requests
    let host_header = headers.get("host").and_then(|h| h.to_str().ok()).unwrap_or("no-host");
    info!("Proxy request: {} {} (Host: {})", method, path, host_header);

    // Parse project name from path (/{project_name}/...) for fallback
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    // Determine project name and routing mode
    let (project_name, is_subdomain_routing) = if let Some(host) = headers.get("host") {
        if let Ok(host_str) = host.to_str() {
            // Extract hostname without port: projectname-app.albl.cloud:9999 -> projectname-app.albl.cloud
            let hostname = host_str.split(':').next().unwrap_or(host_str);

            // Check if subdomain routing should be used
            if let Some(ref base_domain) = state.base_domain {
                // Build the pattern to match: -app.{base_domain}
                let app_suffix = format!("-app.{}", base_domain);

                if hostname.ends_with(&app_suffix) {
                    // Extract project name: projectname-app.albl.cloud -> projectname
                    let project_name = hostname.trim_end_matches(&app_suffix);
                    info!("Subdomain routing: {} -> project '{}'", hostname, project_name);
                    (project_name.to_string(), true)
                } else {
                    // Fallback to path-based routing
                    if parts.is_empty() || parts[0].is_empty() {
                        return Ok(Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .header("Content-Type", "text/plain; charset=utf-8")
                            .body(Full::new(Bytes::from("Not found")))
                            .unwrap());
                    }
                    (parts[0].to_string(), false)
                }
            } else {
                // No base_domain set, use path-based routing
                if parts.is_empty() || parts[0].is_empty() {
                    return Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Full::new(Bytes::from("Not found")))
                        .unwrap());
                }
                (parts[0].to_string(), false)
            }
        } else {
            // Cannot parse host header, fallback to path-based
            if parts.is_empty() || parts[0].is_empty() {
                return Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header("Content-Type", "text/plain; charset=utf-8")
                    .body(Full::new(Bytes::from("Not found")))
                    .unwrap());
            }
            (parts[0].to_string(), false)
        }
    } else {
        // No host header, fallback to path-based
        if parts.is_empty() || parts[0].is_empty() {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from("Not found")))
                .unwrap());
        }
        (parts[0].to_string(), false)
    };

    // Get project from database
    let project = match state.db.get_project_by_name(&project_name).await {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to get project {}: {}", project_name, e);
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "text/plain; charset=utf-8")
                .body(Full::new(Bytes::from("Project not found")))
                .unwrap());
        }
    };

    // Determine target port based on active slot
    let target_port = match project.active_slot {
        Slot::Blue => project.blue_port,
        Slot::Green => project.green_port,
    };

    // Determine target path based on routing mode
    let target_path = if is_subdomain_routing {
        // Subdomain routing: keep full path (/api/users -> /api/users)
        path.to_string()
    } else {
        // Path-based routing: remove first segment (/project1/api/users -> /api/users)
        if parts.len() > 1 {
            format!("/{}", parts[1..].join("/"))
        } else {
            "/".to_string()
        }
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
    info!("→ Forwarding to backend: {}", target_uri);
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
                .header("Content-Type", "text/plain; charset=utf-8")
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
            warn!("✗ Backend request failed: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("Content-Type", "text/plain; charset=utf-8")
                .body(Full::new(Bytes::from("Service unavailable")))
                .unwrap());
        }
    };

    // Convert response
    let status = response.status();
    let headers = response.headers().clone();
    info!("← Backend response: {} (headers: {})", status, headers.len());
    let body = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            warn!("Failed to read response body: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("Content-Type", "text/plain; charset=utf-8")
                .body(Full::new(Bytes::from("Error reading response")))
                .unwrap());
        }
    };

    // Build response with headers copied from backend
    let mut response_builder = Response::builder().status(status.as_u16());

    // Copy all headers from backend response
    let mut header_count = 0;
    for (name, value) in headers.iter() {
        match value.to_str() {
            Ok(value_str) => {
                response_builder = response_builder.header(name.as_str(), value_str);
                if name == "content-type" {
                    info!("  → Copying Content-Type: {}", value_str);
                }
                header_count += 1;
            }
            Err(e) => {
                warn!("Failed to convert header {} to string: {}", name, e);
            }
        }
    }
    info!("← Sending response to client: {} ({} headers)", status, header_count);

    Ok(response_builder
        .body(Full::new(body))
        .unwrap())
}
