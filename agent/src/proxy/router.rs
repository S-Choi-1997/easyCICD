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
use uuid::Uuid;

use crate::db::models::{Slot, ContainerStatus};
use crate::state::AppContext;
use crate::application::ports::repositories::{ProjectRepository, ContainerRepository};
use crate::infrastructure::logging::{TraceContext, Timer};

// Helper to create error responses safely
fn error_response(status: StatusCode, message: &str) -> Result<Response<Full<Bytes>>, hyper::Error> {
    match Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(Full::new(Bytes::from(message.to_string())))
    {
        Ok(response) => Ok(response),
        Err(e) => {
            warn!("Failed to build error response: {:?}", e);
            // Fallback to minimal response
            Ok(Response::new(Full::new(Bytes::from(message.to_string()))))
        }
    }
}

pub async fn run_reverse_proxy(context: AppContext) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await?;

    info!("Reverse proxy listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let ctx = context.clone();

        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        let ctx = ctx.clone();
                        async move { handle_request(req, ctx).await }
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
    ctx: AppContext,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let headers = req.headers().clone();

    // Generate trace ID for this proxy request
    let trace_id = format!("proxy-{}", Uuid::new_v4());
    let timer = Timer::start();

    // Log all incoming requests
    let host_header = headers.get("host").and_then(|h| h.to_str().ok()).unwrap_or("no-host");
    ctx.logger.api_entry(&trace_id, method.as_str(), &format!("PROXY {}", path), &format!("Host: {}", host_header));

    // Parse name from path (/{name}/...) for fallback
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    // Routing result: either Project or Container
    enum RouteTarget {
        Project { name: String, is_subdomain: bool },
        Container { name: String, is_subdomain: bool },
    }

    // Determine routing target based on Host header and subdomain pattern
    let route_target = if let Some(host) = headers.get("host") {
        if let Ok(host_str) = host.to_str() {
            // Extract hostname without port: name.albl.cloud:9999 -> name.albl.cloud
            let hostname = host_str.split(':').next().unwrap_or(host_str);

            // Check if subdomain routing should be used
            if let Some(ref base_domain) = ctx.base_domain {
                let domain_suffix = format!(".{}", base_domain);

                if hostname.ends_with(&domain_suffix) {
                    let subdomain = hostname.trim_end_matches(&domain_suffix);

                    // Check for project pattern: {name}-app
                    if subdomain.ends_with("-app") {
                        let project_name = subdomain.trim_end_matches("-app");
                        info!("Subdomain routing: {} -> project '{}'", hostname, project_name);
                        RouteTarget::Project { name: project_name.to_string(), is_subdomain: true }
                    } else {
                        // Standalone container pattern: {name}
                        info!("Subdomain routing: {} -> container '{}'", hostname, subdomain);
                        RouteTarget::Container { name: subdomain.to_string(), is_subdomain: true }
                    }
                } else {
                    // Fallback to path-based routing
                    if parts.is_empty() || parts[0].is_empty() {
                        return error_response(StatusCode::NOT_FOUND, "Not found");
                    }
                    // Assume path-based is for projects
                    RouteTarget::Project { name: parts[0].to_string(), is_subdomain: false }
                }
            } else {
                // No base_domain set, use path-based routing
                if parts.is_empty() || parts[0].is_empty() {
                    return error_response(StatusCode::NOT_FOUND, "Not found");
                }
                RouteTarget::Project { name: parts[0].to_string(), is_subdomain: false }
            }
        } else {
            // Cannot parse host header, fallback to path-based
            if parts.is_empty() || parts[0].is_empty() {
                return error_response(StatusCode::NOT_FOUND, "Not found");
            }
            RouteTarget::Project { name: parts[0].to_string(), is_subdomain: false }
        }
    } else {
        // No host header, fallback to path-based
        if parts.is_empty() || parts[0].is_empty() {
            return error_response(StatusCode::NOT_FOUND, "Not found");
        }
        RouteTarget::Project { name: parts[0].to_string(), is_subdomain: false }
    };

    // Route to target (either project or standalone container)
    let (target_container_name, target_port, is_subdomain_routing) = match route_target {
        RouteTarget::Project { name: project_name, is_subdomain } => {
            // Get project from database
            info!("[{}] Routing request → project: '{}'", trace_id, project_name);
            ctx.logger.repo_call(&trace_id, "Proxy", "ProjectRepo", "get_by_name");

            let project = match ctx.project_repo.get_by_name(&project_name).await {
                Ok(Some(p)) => {
                    info!("[{}] Project found: id={}, active_slot={:?}, runtime_port={}", trace_id, p.id, p.active_slot, p.runtime_port);
                    p
                }
                Ok(None) => {
                    warn!("[{}] Project not found: {}", trace_id, project_name);
                    ctx.logger.api_exit(&trace_id, method.as_str(), &format!("PROXY {}", path), timer.elapsed_ms(), 404);
                    return error_response(StatusCode::NOT_FOUND, "Project not found");
                }
                Err(e) => {
                    warn!("[{}] Failed to get project {}: {}", trace_id, project_name, e);
                    ctx.logger.api_exit(&trace_id, method.as_str(), &format!("PROXY {}", path), timer.elapsed_ms(), 500);
                    return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal error");
                }
            };

            // Determine container name and internal port based on active slot
            let container_name = match project.active_slot {
                Slot::Blue => format!("project-{}-blue", project.id),
                Slot::Green => format!("project-{}-green", project.id),
            };

            (container_name, project.runtime_port, is_subdomain)
        }

        RouteTarget::Container { name: container_name, is_subdomain } => {
            // Get standalone container from database
            info!("[{}] Routing request → container: '{}'", trace_id, container_name);
            ctx.logger.repo_call(&trace_id, "Proxy", "ContainerRepo", "get_by_name");

            let container = match ctx.container_repo.get_by_name(&container_name).await {
                Ok(Some(c)) => {
                    info!("[{}] Container found: id={}, status={:?}, port={}", trace_id, c.id, c.status, c.port);
                    c
                }
                Ok(None) => {
                    warn!("[{}] Container not found: {}", trace_id, container_name);
                    ctx.logger.api_exit(&trace_id, method.as_str(), &format!("PROXY {}", path), timer.elapsed_ms(), 404);
                    return error_response(StatusCode::NOT_FOUND, "Container not found");
                }
                Err(e) => {
                    warn!("[{}] Failed to get container {}: {}", trace_id, container_name, e);
                    ctx.logger.api_exit(&trace_id, method.as_str(), &format!("PROXY {}", path), timer.elapsed_ms(), 500);
                    return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal error");
                }
            };

            // Check if container is running
            if container.status != ContainerStatus::Running {
                warn!("[{}] Container {} is not running (status: {:?})", trace_id, container_name, container.status);
                ctx.logger.api_exit(&trace_id, method.as_str(), &format!("PROXY {}", path), timer.elapsed_ms(), 503);
                return error_response(StatusCode::SERVICE_UNAVAILABLE, "Container is not running");
            }

            // Use the actual Docker container name format: container-{name}
            let docker_container_name = format!("container-{}", container.name);

            // Use container_port if specified, otherwise use port
            let target_port = container.container_port.unwrap_or(container.port);

            (docker_container_name, target_port, is_subdomain)
        }
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

    // Preserve query string - route directly to container by name on the same Docker network
    let target_uri = if let Some(query) = req.uri().query() {
        format!("http://{}:{}{}?{}", target_container_name, target_port, target_path, query)
    } else {
        format!("http://{}:{}{}", target_container_name, target_port, target_path)
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

    let reqwest_method = match *req.method() {
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
    let mut req_builder = client.request(reqwest_method, &target_uri);

    // Copy headers except Host
    for (name, value) in headers.iter() {
        if name != "host" && name != "content-length" {
            if let Ok(value_str) = value.to_str() {
                req_builder = req_builder.header(name.as_str(), value_str);
            }
        }
    }

    info!("[{}] Forwarding to backend: {}", trace_id, target_uri);

    let response = match req_builder
        .body(body_bytes.to_vec())
        .send()
        .await
    {
        Ok(res) => res,
        Err(e) => {
            warn!("[{}] Backend request failed: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, method.as_str(), &format!("PROXY {}", path), timer.elapsed_ms(), 502);
            return error_response(StatusCode::BAD_GATEWAY, "Service unavailable");
        }
    };

    // Convert response
    let status = response.status();
    let headers = response.headers().clone();
    info!("[{}] Backend response: {}", trace_id, status);

    let body = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            warn!("[{}] Failed to read response body: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, method.as_str(), &format!("PROXY {}", path), timer.elapsed_ms(), 502);
            return error_response(StatusCode::BAD_GATEWAY, "Error reading response");
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
    ctx.logger.api_exit(&trace_id, method.as_str(), &format!("PROXY {}", path), timer.elapsed_ms(), status.as_u16());

    match response_builder.body(Full::new(body.clone())) {
        Ok(response) => Ok(response),
        Err(e) => {
            warn!("[{}] Failed to build final response: {:?}", trace_id, e);
            // Fallback to simple response
            Ok(Response::new(Full::new(body)))
        }
    }
}
