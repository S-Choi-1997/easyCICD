use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, State},
    response::Response,
};
use bollard::container::LogOutput;
use bollard::exec::StartExecResults;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::application::ports::repositories::ContainerRepository;
use crate::state::AppContext;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum TerminalInput {
    #[serde(rename = "input")]
    Input { data: String },
    #[serde(rename = "resize")]
    Resize { rows: u16, cols: u16 },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum TerminalOutput {
    #[serde(rename = "output")]
    Output { data: String },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "connected")]
    Connected,
}

/// WebSocket handler for container terminal
/// Route: /api/containers/{container_db_id}/terminal
pub async fn container_terminal(
    State(ctx): State<AppContext>,
    Path(container_db_id): Path<i64>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_terminal_session(socket, ctx, container_db_id))
}

async fn handle_terminal_session(
    socket: WebSocket,
    ctx: AppContext,
    container_db_id: i64,
) {
    info!("Terminal WebSocket connected for container DB ID: {}", container_db_id);

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // 1. Get container info from DB
    let container = match ctx.container_repo.get(container_db_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            let msg = serde_json::to_string(&TerminalOutput::Error {
                message: "Container not found".to_string(),
            }).unwrap();
            let _ = ws_sender.send(Message::Text(msg.into())).await;
            return;
        }
        Err(e) => {
            let msg = serde_json::to_string(&TerminalOutput::Error {
                message: format!("DB error: {}", e),
            }).unwrap();
            let _ = ws_sender.send(Message::Text(msg.into())).await;
            return;
        }
    };

    // Get Docker container name
    let docker_container_name = format!("container-{}", container.name);

    // 2. Create exec session (stty -echo로 서버 에코 비활성화, 클라이언트가 로컬 에코 담당)
    let (exec_id, exec_output) = match ctx.docker.create_exec_session(
        &docker_container_name,
        vec!["/bin/sh".to_string(), "-c".to_string(), "stty -echo; exec /bin/sh".to_string()],
    ).await {
        Ok(result) => result,
        Err(e) => {
            let msg = serde_json::to_string(&TerminalOutput::Error {
                message: format!("Failed to create exec: {}", e),
            }).unwrap();
            let _ = ws_sender.send(Message::Text(msg.into())).await;
            return;
        }
    };

    // Send connected message
    let connected_msg = serde_json::to_string(&TerminalOutput::Connected).unwrap();
    if ws_sender.send(Message::Text(connected_msg.into())).await.is_err() {
        return;
    }

    // 3. Bridge WebSocket <-> Docker exec stream
    match exec_output {
        StartExecResults::Attached { mut output, mut input } => {
            let docker = ctx.docker.clone();
            let exec_id_clone = exec_id.clone();

            // Task: Docker stdout -> WebSocket
            let output_task = tokio::spawn(async move {
                while let Some(result) = output.next().await {
                    match result {
                        Ok(log_output) => {
                            let data = match log_output {
                                LogOutput::StdOut { message } => String::from_utf8_lossy(&message).to_string(),
                                LogOutput::StdErr { message } => String::from_utf8_lossy(&message).to_string(),
                                LogOutput::Console { message } => String::from_utf8_lossy(&message).to_string(),
                                _ => String::new(),
                            };
                            if !data.is_empty() {
                                let msg = serde_json::to_string(&TerminalOutput::Output { data }).unwrap();
                                if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Exec output error: {}", e);
                            break;
                        }
                    }
                }
            });

            // Task: WebSocket -> Docker stdin
            let input_task = tokio::spawn(async move {
                while let Some(Ok(msg)) = ws_receiver.next().await {
                    match msg {
                        Message::Text(text) => {
                            if let Ok(terminal_input) = serde_json::from_str::<TerminalInput>(&text) {
                                match terminal_input {
                                    TerminalInput::Input { data } => {
                                        if input.write_all(data.as_bytes()).await.is_err() {
                                            break;
                                        }
                                        if input.flush().await.is_err() {
                                            break;
                                        }
                                    }
                                    TerminalInput::Resize { rows, cols } => {
                                        let _ = docker.resize_exec_tty(&exec_id_clone, rows, cols).await;
                                    }
                                }
                            }
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
            });

            // Wait for either task to finish
            tokio::select! {
                _ = output_task => {}
                _ = input_task => {}
            }
        }
        StartExecResults::Detached => {
            let msg = serde_json::to_string(&TerminalOutput::Error {
                message: "Exec detached unexpectedly".to_string(),
            }).unwrap();
            let _ = ws_sender.send(Message::Text(msg.into())).await;
        }
    }

    info!("Terminal session ended for container {}", container_db_id);
}
