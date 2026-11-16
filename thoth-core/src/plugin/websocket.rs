//! Host-side WebSocket connection manager for the plugin `websocket` import.
//!
//! Plugins are sandboxed and can't open sockets, so the host owns each
//! WebSocket connection (via `tokio-tungstenite`) and streams frames back to
//! the plugin through the async `handle-event` path — mirroring the `http`
//! `submit` model. Each connection runs as a task on a shared multi-thread
//! runtime; the plugin controls it with a command channel and receives
//! lifecycle + message events on an mpsc channel drained by the app each frame.

use std::sync::LazyLock;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};

/// Dedicated runtime for WebSocket tasks (kept off the blocking HTTP threads).
static WS_RT: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build websocket runtime")
});

/// Keepalive ping cadence.
const PING_INTERVAL: Duration = Duration::from_secs(30);

/// Bound the initial handshake so a dead host doesn't leave the task hanging.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// A lifecycle / message event delivered from a connection task to the app.
#[derive(Debug)]
pub enum WsEvent {
    Open,
    Text(String),
    Binary(Vec<u8>),
    Error(String),
    Closed { code: u16, reason: String },
}

/// A command the plugin issues on an open connection.
pub enum WsCommand {
    Text(String),
    Binary(Vec<u8>),
    Close,
}

/// Spawn a WebSocket connection task for `url` (already policy-approved by the
/// caller). Lifecycle + messages are sent on `event_tx` tagged with `conn_id`;
/// the returned sender drives `send`/`close`. Dropping the sender closes it.
pub fn spawn(
    url: String,
    headers: Vec<(String, String)>,
    conn_id: String,
    event_tx: std::sync::mpsc::Sender<(String, WsEvent)>,
) -> UnboundedSender<WsCommand> {
    let (cmd_tx, mut cmd_rx) = unbounded_channel::<WsCommand>();

    WS_RT.spawn(async move {
        let mut request = match url.into_client_request() {
            Ok(req) => req,
            Err(e) => {
                emit(&event_tx, &conn_id, WsEvent::Error(e.to_string()));
                return;
            }
        };
        // Fail fast on a malformed header rather than silently dropping it
        // (e.g. a bad Authorization value would otherwise connect unauthed).
        for (k, v) in &headers {
            match (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                (Ok(name), Ok(val)) => {
                    request.headers_mut().insert(name, val);
                }
                _ => {
                    emit(
                        &event_tx,
                        &conn_id,
                        WsEvent::Error(format!("invalid header: {k}")),
                    );
                    return;
                }
            }
        }

        let connect = tokio_tungstenite::connect_async(request);
        let ws = match tokio::time::timeout(CONNECT_TIMEOUT, connect).await {
            Ok(Ok((ws, _resp))) => ws,
            Ok(Err(e)) => {
                emit(&event_tx, &conn_id, WsEvent::Error(e.to_string()));
                return;
            }
            Err(_) => {
                emit(
                    &event_tx,
                    &conn_id,
                    WsEvent::Error("connection timed out".to_string()),
                );
                return;
            }
        };
        emit(&event_tx, &conn_id, WsEvent::Open);

        let (mut write, mut read) = ws.split();
        let mut ping = tokio::time::interval(PING_INTERVAL);
        ping.tick().await; // consume the immediate first tick

        loop {
            tokio::select! {
                incoming = read.next() => match incoming {
                    Some(Ok(Message::Text(t))) => {
                        emit(&event_tx, &conn_id, WsEvent::Text(t));
                    }
                    Some(Ok(Message::Binary(b))) => {
                        emit(&event_tx, &conn_id, WsEvent::Binary(b));
                    }
                    Some(Ok(Message::Close(frame))) => {
                        let (code, reason) = frame
                            .map(|c| (u16::from(c.code), c.reason.to_string()))
                            .unwrap_or((1005, String::new()));
                        emit(&event_tx, &conn_id, WsEvent::Closed { code, reason });
                        break;
                    }
                    // Ping/Pong are handled by tungstenite; ignore other frames.
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        emit(&event_tx, &conn_id, WsEvent::Error(e.to_string()));
                        break;
                    }
                    None => {
                        emit(&event_tx, &conn_id, WsEvent::Closed {
                            code: 1006,
                            reason: "connection closed".to_string(),
                        });
                        break;
                    }
                },
                cmd = cmd_rx.recv() => match cmd {
                    Some(WsCommand::Text(t)) => {
                        if write.send(Message::Text(t)).await.is_err() {
                            break;
                        }
                    }
                    Some(WsCommand::Binary(b)) => {
                        if write.send(Message::Binary(b)).await.is_err() {
                            break;
                        }
                    }
                    // Explicit close, or the plugin dropped the sender.
                    Some(WsCommand::Close) | None => {
                        let _ = write.send(Message::Close(None)).await;
                        emit(&event_tx, &conn_id, WsEvent::Closed {
                            code: 1000,
                            reason: "closed by client".to_string(),
                        });
                        break;
                    }
                },
                _ = ping.tick() => {
                    if write.send(Message::Ping(Vec::new())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    cmd_tx
}

/// Deliver an event to the app and wake the UI to render it.
fn emit(tx: &std::sync::mpsc::Sender<(String, WsEvent)>, conn_id: &str, event: WsEvent) {
    let _ = tx.send((conn_id.to_string(), event));
    if let Some(ctx) = crate::EGUI_CTX.get() {
        ctx.request_repaint();
    }
}
