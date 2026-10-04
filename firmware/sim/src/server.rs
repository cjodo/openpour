//! The firmware's HTTP API and WebSocket (see firmware/esp32/src/net.rs),
//! plus `/api/sim` for faults and simulated hardware state. Handlers only
//! queue messages; the simulation thread owns the machine.

use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tower_http::services::{ServeDir, ServeFile};

/// From the HTTP side to the simulation thread.
pub enum ToSim {
    /// A JSON command line, exactly as the firmware's main loop receives it.
    Command(String),
    /// Fault switches and simulated-hardware changes from `POST /api/sim`.
    Sim(Value),
    /// A WebSocket client connected and wants a status right away.
    WantStatus,
}

/// Read by handlers, refreshed by the simulation thread.
#[derive(Default)]
pub struct Snapshot {
    pub settings: String,
    pub recipes: String,
    pub sim: String,
}

#[derive(Clone)]
pub struct Shared {
    pub to_sim: SyncSender<ToSim>,
    pub out: broadcast::Sender<String>,
    pub snapshot: Arc<Mutex<Snapshot>>,
}

fn json_response(status: StatusCode, body: String) -> Response {
    (status, [("Content-Type", "application/json")], body).into_response()
}

fn ok() -> Response {
    json_response(StatusCode::OK, r#"{"ok":true}"#.into())
}

/// Same limits and replies as `forward_post` in net.rs.
fn forward(s: &Shared, cmd: &str, body: &str, max: usize, want_array: bool) -> Response {
    let data = (body.len() <= max).then(|| serde_json::from_str::<Value>(body).ok()).flatten();
    let Some(data) = data else {
        return json_response(StatusCode::BAD_REQUEST, r#"{"error":"Expected a JSON body"}"#.into());
    };
    if want_array && !data.is_array() {
        return json_response(StatusCode::BAD_REQUEST, r#"{"error":"Expected an array of recipes"}"#.into());
    }
    if s.to_sim.try_send(ToSim::Command(json!({ "cmd": cmd, "data": data }).to_string())).is_err() {
        return json_response(StatusCode::SERVICE_UNAVAILABLE, r#"{"error":"Busy, try again"}"#.into());
    }
    ok()
}

async fn ws(ws: WebSocketUpgrade, State(s): State<Shared>) -> Response {
    ws.on_upgrade(move |socket| client(socket, s))
}

async fn client(mut socket: WebSocket, s: Shared) {
    let mut out = s.out.subscribe();
    let _ = s.to_sim.try_send(ToSim::WantStatus);
    loop {
        tokio::select! {
            msg = socket.recv() => match msg {
                Some(Ok(Message::Text(line))) => {
                    let _ = s.to_sim.try_send(ToSim::Command(line.to_string()));
                }
                Some(Ok(_)) => {}
                _ => break,
            },
            line = out.recv() => match line {
                Ok(line) => {
                    if socket.send(Message::Text(line.into())).await.is_err() {
                        break;
                    }
                }
                // A slow client misses messages, as on the ESP32.
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => break,
            },
        }
    }
}

pub fn router(s: Shared, web: PathBuf) -> Router {
    const MAX_SETTINGS_BODY: usize = 4 * 1024;
    const MAX_RECIPES_BODY: usize = 32 * 1024;

    // Unknown paths get the single-page app, as on the ESP32.
    let app = ServeDir::new(&web).fallback(ServeFile::new(web.join("index.html")));
    Router::new()
        .route("/ws", any(ws))
        .route(
            "/api/settings",
            get(|State(s): State<Shared>| async move {
                json_response(StatusCode::OK, s.snapshot.lock().unwrap().settings.clone())
            })
            .post(|State(s): State<Shared>, body: String| async move {
                forward(&s, "settings", &body, MAX_SETTINGS_BODY, false)
            }),
        )
        .route(
            "/api/recipes",
            get(|State(s): State<Shared>| async move {
                json_response(StatusCode::OK, s.snapshot.lock().unwrap().recipes.clone())
            })
            .post(|State(s): State<Shared>, body: String| async move {
                forward(&s, "saveRecipes", &body, MAX_RECIPES_BODY, true)
            }),
        )
        .route(
            "/api/sim",
            get(|State(s): State<Shared>| async move {
                json_response(StatusCode::OK, s.snapshot.lock().unwrap().sim.clone())
            })
            .post(|State(s): State<Shared>, body: String| async move {
                match serde_json::from_str::<Value>(&body) {
                    Ok(v) if v.is_object() => {
                        let _ = s.to_sim.try_send(ToSim::Sim(v));
                        ok()
                    }
                    _ => json_response(StatusCode::BAD_REQUEST, r#"{"error":"Expected a JSON object"}"#.into()),
                }
            }),
        )
        .route(
            "/api/{*rest}",
            any(|| async { json_response(StatusCode::NOT_FOUND, r#"{"error":"Not found"}"#.into()) }),
        )
        .fallback_service(app)
        .with_state(s)
}
