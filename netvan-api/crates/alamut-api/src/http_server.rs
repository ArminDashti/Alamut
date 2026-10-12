use anyhow::Result;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderValue, Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use alamut_collectors::ip_info;
use alamut_collectors::ping::{self, PingLineEvent};
use alamut_collectors::speedtest::{self, SpeedtestProgress};
use alamut_collectors::traceroute::{self, TracerouteHopEvent};
use alamut_collectors::CollectorEngine;
use alamut_core::db::Database;
use alamut_core::ipc::{RpcRequest, RpcResponse};
use alamut_core::paths::{self, DEFAULT_BIND};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceExt;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<CollectorEngine>,
}

pub async fn run_standalone() -> Result<()> {
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    run(shutdown).await
}

pub async fn run(shutdown: impl Future<Output = ()> + Send + 'static) -> Result<()> {
    paths::ensure_data_dir()?;
    let db = Database::open_default()?;
    let engine = CollectorEngine::new(db)?;
    engine.clone().start_background().await;

    let webui_dir = paths::webui_dir();
    if let Some(dir) = &webui_dir {
        info!("serving web UI from {}", dir.display());
    } else {
        info!("web UI not found; only /api endpoints will be available");
    }

    let state = AppState { engine };

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any)
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            let Ok(s) = origin.to_str() else {
                return false;
            };
            s.starts_with("http://127.0.0.1:")
                || s.starts_with("http://localhost:")
                || s == "http://pc-armin"
                || s.starts_with("http://pc-armin:")
                || s == "http://alamut.local"
                || s.starts_with("http://alamut.local:")
                || s == "http://alamut-api.local"
                || s.starts_with("http://alamut-api.local:")
                || s == "null"
        }));

    let mut app = Router::new()
        .route("/api/health", get(health))
        .route("/api/data-dir", get(data_dir))
        .route("/api/rpc", post(rpc))
        .route("/api/ws/tools", get(ws_tools))
        .with_state(state.clone())
        .layer(cors);

    if let Some(dir) = webui_dir {
        let serve_dir = ServeDir::new(dir.clone());
        let index_path = dir.join("index.html");
        app = app.fallback(move |req: Request<Body>| {
            let index_path = index_path.clone();
            let serve_dir = serve_dir.clone();
            async move {
                let path = req.uri().path().to_string();
                if path.starts_with("/api/") {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(json!({ "error": "not found", "path": path })),
                    )
                        .into_response();
                }
                let has_ext = path
                    .rsplit('/')
                    .next()
                    .and_then(|f| f.rsplit_once('.'))
                    .map(|(name, _)| !name.is_empty())
                    .unwrap_or(false);
                match serve_dir.clone().oneshot(req).await {
                    Ok(resp) => {
                        if has_ext {
                            return resp.into_response();
                        }
                        if resp.status() == StatusCode::NOT_FOUND {
                            if let Ok(body) = std::fs::read(&index_path) {
                                return Response::builder()
                                    .status(StatusCode::OK)
                                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                                    .body(Body::from(body))
                                    .unwrap_or_else(|_| {
                                        (
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                            "index.html read error",
                                        )
                                            .into_response()
                                    });
                            }
                        }
                        resp.into_response()
                    }
                    Err(_) => {
                        if !has_ext {
                            if let Ok(body) = std::fs::read(&index_path) {
                                return Response::builder()
                                    .status(StatusCode::OK)
                                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                                    .body(Body::from(body))
                                    .unwrap_or_else(|_| {
                                        (
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                            "index.html read error",
                                        )
                                            .into_response()
                                    });
                            }
                        }
                        (StatusCode::NOT_FOUND, "not found").into_response()
                    }
                }
            }
        });
    } else {
        app = app.fallback(move |req: Request<Body>| async move {
            let path = req.uri().path().to_string();
            if path.starts_with("/api/") {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": "not found", "path": path })),
                )
                    .into_response();
            }
            let msg = "Alamut API running. Build web UI with: cd alamut-webui ; npm install ; npm run build\n\
                     Then restart the service. Access the UI at: http://alamut.local";
            (StatusCode::OK, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], msg).into_response()
        });
    }

    let bind = std::env::var("ALAMUT_API_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let addr: SocketAddr = bind.parse()?;
    info!("Alamut listening on http://{addr} (UI: http://alamut.local)");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true, "service": "Alamut" }))
}

async fn data_dir() -> impl IntoResponse {
    match paths::ensure_data_dir() {
        Ok(p) => Json(json!({ "path": p.display().to_string() })).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn rpc(State(state): State<AppState>, Json(req): Json<RpcRequest>) -> impl IntoResponse {
    let response = state.engine.handle(req).await;
    Json(response)
}

async fn ws_tools(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_tools_socket(socket, state))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMsg {
    PingLive {
        target: String,
        count: Option<u32>,
        packet_size: Option<u32>,
    },
    TracerouteLive {
        target: String,
        max_hops: Option<u8>,
    },
    SpeedtestLive {
        nic_id: Option<String>,
        server_id: Option<String>,
        accept_eula: bool,
    },
    CancelSpeedtest,
    LookupIpInfo {
        ip: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMsg {
    PingLine { event: PingLineEvent },
    PingDone { result: serde_json::Value },
    TracerouteHop { event: TracerouteHopEvent },
    TracerouteDone { result: serde_json::Value },
    SpeedtestProgress { event: SpeedtestProgress },
    SpeedtestDone { result: serde_json::Value },
    IpInfo { ip: String, info: serde_json::Value },
    Error { message: String },
}

async fn handle_tools_socket(socket: WebSocket, state: AppState) {
    let (mut tx, mut rx) = socket.split();

    while let Some(Ok(msg)) = rx.next().await {
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };

        let client: ClientMsg = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                let _ = send_json(
                    &mut tx,
                    &ServerMsg::Error {
                        message: format!("bad message: {e}"),
                    },
                )
                .await;
                continue;
            }
        };

        match client {
            ClientMsg::CancelSpeedtest => {
                if let Err(e) = speedtest::cancel_speedtest() {
                    let _ = send_json(
                        &mut tx,
                        &ServerMsg::Error {
                            message: e.to_string(),
                        },
                    )
                    .await;
                }
            }
            ClientMsg::LookupIpInfo { ip } => match ip_info::lookup_ip_info(&ip).await {
                Ok(info) => {
                    let _ = send_json(
                        &mut tx,
                        &ServerMsg::IpInfo {
                            ip,
                            info: serde_json::to_value(info).unwrap_or(json!(null)),
                        },
                    )
                    .await;
                }
                Err(e) => {
                    let _ = send_json(
                        &mut tx,
                        &ServerMsg::Error {
                            message: e.to_string(),
                        },
                    )
                    .await;
                }
            },
            ClientMsg::PingLive {
                target,
                count,
                packet_size,
            } => {
                let n = count.unwrap_or(4).clamp(1, 100);
                let size = packet_size.map(|s| s.min(65500));
                let (line_tx, mut line_rx) = tokio::sync::mpsc::unbounded_channel::<PingLineEvent>();
                let target_clone = target.clone();
                let join = tokio::spawn(async move {
                    ping::ping_with_progress(
                        &target_clone,
                        None,
                        None,
                        n,
                        size,
                        move |ev| {
                            let _ = line_tx.send(ev);
                        },
                    )
                    .await
                });

                while let Some(ev) = line_rx.recv().await {
                    if send_json(&mut tx, &ServerMsg::PingLine { event: ev })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }

                match join.await {
                    Ok(Ok(result)) => {
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::PingDone {
                                result: serde_json::to_value(result).unwrap_or(json!(null)),
                            },
                        )
                        .await;
                    }
                    Ok(Err(e)) => {
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::Error {
                                message: e.to_string(),
                            },
                        )
                        .await;
                    }
                    Err(e) => {
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::Error {
                                message: e.to_string(),
                            },
                        )
                        .await;
                    }
                }
            }
            ClientMsg::TracerouteLive { target, max_hops } => {
                let hops = max_hops.unwrap_or(30).clamp(1, 64);
                let (hop_tx, mut hop_rx) =
                    tokio::sync::mpsc::unbounded_channel::<TracerouteHopEvent>();
                let target_clone = target.clone();
                let join = tokio::spawn(async move {
                    traceroute::traceroute_with_progress(&target_clone, None, hops, move |ev| {
                        let _ = hop_tx.send(ev);
                    })
                    .await
                });

                while let Some(ev) = hop_rx.recv().await {
                    if send_json(&mut tx, &ServerMsg::TracerouteHop { event: ev })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }

                match join.await {
                    Ok(Ok(result)) => {
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::TracerouteDone {
                                result: serde_json::to_value(result).unwrap_or(json!(null)),
                            },
                        )
                        .await;
                    }
                    Ok(Err(e)) => {
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::Error {
                                message: e.to_string(),
                            },
                        )
                        .await;
                    }
                    Err(e) => {
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::Error {
                                message: e.to_string(),
                            },
                        )
                        .await;
                    }
                }
            }
            ClientMsg::SpeedtestLive {
                nic_id,
                server_id,
                accept_eula,
            } => {
                if accept_eula {
                    let _ = state.engine.handle(RpcRequest::AcceptSpeedtestEula).await;
                }
                let settings = match state.engine.handle(RpcRequest::GetSettings).await {
                    RpcResponse::Settings(s) => s,
                    RpcResponse::Error { message } => {
                        let _ = send_json(&mut tx, &ServerMsg::Error { message }).await;
                        continue;
                    }
                    _ => {
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::Error {
                                message: "failed to load settings".into(),
                            },
                        )
                        .await;
                        continue;
                    }
                };
                if !settings.speedtest_eula_accepted && !accept_eula {
                    let _ = send_json(
                        &mut tx,
                        &ServerMsg::Error {
                            message: "Accept Ookla Speedtest EULA/GDPR first".into(),
                        },
                    )
                    .await;
                    continue;
                }

                let (prog_tx, mut prog_rx) =
                    tokio::sync::mpsc::unbounded_channel::<SpeedtestProgress>();
                let cli_path = settings.speedtest_cli_path.clone();
                let join = tokio::spawn(async move {
                    speedtest::run_speedtest_with_progress(
                        nic_id,
                        server_id,
                        cli_path,
                        true,
                        move |p| {
                            let _ = prog_tx.send(p);
                        },
                    )
                    .await
                });

                while let Some(ev) = prog_rx.recv().await {
                    if send_json(&mut tx, &ServerMsg::SpeedtestProgress { event: ev })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }

                match join.await {
                    Ok(Ok(mut result)) => {
                        if let Ok(id) = state.engine.db().insert_speedtest(&result) {
                            result.id = id;
                        }
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::SpeedtestProgress {
                                event: SpeedtestProgress {
                                    phase: "done".into(),
                                    download_mbps: Some(result.download_mbps),
                                    upload_mbps: Some(result.upload_mbps),
                                    ping_ms: Some(result.ping_ms),
                                    server_name: result.server_name.clone(),
                                },
                            },
                        )
                        .await;
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::SpeedtestDone {
                                result: serde_json::to_value(result).unwrap_or(json!(null)),
                            },
                        )
                        .await;
                    }
                    Ok(Err(e)) => {
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::Error {
                                message: e.to_string(),
                            },
                        )
                        .await;
                    }
                    Err(e) => {
                        let _ = send_json(
                            &mut tx,
                            &ServerMsg::Error {
                                message: e.to_string(),
                            },
                        )
                        .await;
                    }
                }
            }
        }
    }
}

async fn send_json<T: Serialize>(
    tx: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    msg: &T,
) -> Result<(), axum::Error> {
    let text = serde_json::to_string(msg).unwrap_or_else(|_| {
        json!({"type":"error","message":"serialize failed"}).to_string()
    });
    tx.send(Message::Text(text.into())).await
}
