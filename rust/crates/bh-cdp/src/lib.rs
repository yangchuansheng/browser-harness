use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type PendingResponse = Result<Value, String>;
type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<PendingResponse>>>>;
type WsWrite = futures_util::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

#[derive(Debug, Clone, PartialEq)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    pub session_id: Option<String>,
}

#[derive(Clone)]
pub struct CdpClient {
    inner: Arc<CdpClientInner>,
}

struct CdpClientInner {
    endpoint: String,
    next_id: AtomicU64,
    pending: PendingMap,
    writer: Mutex<WsWrite>,
}

impl CdpClient {
    pub async fn connect(
        endpoint: impl Into<String>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<CdpEvent>), String> {
        let endpoint = endpoint.into();
        let (stream, _) = connect_async(endpoint.as_str()).await.map_err(|err| {
            format!(
                "CDP WS handshake failed: {err} -- click Allow in Chrome if prompted, then retry"
            )
        })?;
        let (writer, mut reader) = stream.split();
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, events_rx) = mpsc::unbounded_channel();

        let client = Self {
            inner: Arc::new(CdpClientInner {
                endpoint,
                next_id: AtomicU64::new(1),
                pending: pending.clone(),
                writer: Mutex::new(writer),
            }),
        };

        tokio::spawn(async move {
            while let Some(message) = reader.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        handle_message(text.as_ref(), &pending, &events_tx).await;
                    }
                    Ok(Message::Binary(bytes)) => {
                        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                            handle_message(&text, &pending, &events_tx).await;
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                    Err(err) => {
                        fail_pending(&pending, format!("CDP websocket error: {err}")).await;
                        return;
                    }
                    _ => {}
                }
            }
            fail_pending(&pending, "CDP connection closed".to_string()).await;
        });

        Ok((client, events_rx))
    }

    pub fn endpoint(&self) -> &str {
        &self.inner.endpoint
    }

    pub async fn send_raw(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, String> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().await.insert(id, tx);

        let mut payload = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        if let Some(session_id) = session_id {
            payload["sessionId"] = Value::String(session_id.to_string());
        }
        let payload_text = serde_json::to_string(&payload)
            .map_err(|err| format!("serialize CDP request: {err}"))?;

        let send_result = self
            .inner
            .writer
            .lock()
            .await
            .send(Message::Text(payload_text.into()))
            .await;
        if let Err(err) = send_result {
            self.inner.pending.lock().await.remove(&id);
            return Err(format!("send CDP request: {err}"));
        }

        match timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("CDP response channel closed".to_string()),
            Err(_) => {
                self.inner.pending.lock().await.remove(&id);
                Err(format!("timed out waiting for CDP response to {method}"))
            }
        }
    }
}

pub fn is_browser_level_method(method: &str) -> bool {
    method.starts_with("Target.")
}

async fn handle_message(
    text: &str,
    pending: &PendingMap,
    events_tx: &mpsc::UnboundedSender<CdpEvent>,
) {
    let Ok(message) = serde_json::from_str::<Value>(text) else {
        return;
    };

    if let Some(id) = message.get("id").and_then(Value::as_u64) {
        let response = if let Some(error) = message.get("error") {
            Err(error
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| error.to_string()))
        } else {
            Ok(message.get("result").cloned().unwrap_or_else(|| json!({})))
        };

        if let Some(tx) = pending.lock().await.remove(&id) {
            let _ = tx.send(response);
        }
        return;
    }

    if let Some(method) = message.get("method").and_then(Value::as_str) {
        let event = CdpEvent {
            method: method.to_string(),
            params: message.get("params").cloned().unwrap_or_else(|| json!({})),
            session_id: message
                .get("sessionId")
                .and_then(Value::as_str)
                .map(str::to_string),
        };
        let _ = events_tx.send(event);
    }
}

async fn fail_pending(pending: &PendingMap, message: String) {
    let mut pending = pending.lock().await;
    for (_, tx) in pending.drain() {
        let _ = tx.send(Err(message.clone()));
    }
}
