//! Połączenie JSON-RPC klienta MCP nad dowolnym strumieniem (stdio procesu, duplex w testach):
//! korelacja odpowiedzi, limity czasu, obsługa żądań i powiadomień serwera.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use mcp_contract::jsonrpc::{Dialect, Message, RequestId, RpcError, parse_line};
use mcp_contract::protocol::methods;
use mcp_contract::{McpError, McpWarning};
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;

use crate::lines::{DEFAULT_MAX_LINE_BYTES, Line, read_line};

type Pending = HashMap<RequestId, oneshot::Sender<Result<Value, RpcError>>>;
type BoxWriter = Box<dyn AsyncWrite + Send + Unpin>;

/// Maksymalna liczba zapamiętanych ostrzeżeń (starsze są odrzucane).
const MAX_WARNINGS: usize = 256;

struct State {
    pending: Mutex<Pending>,
    warnings: Mutex<Vec<McpWarning>>,
    closed: AtomicBool,
    tools_changed: AtomicBool,
}

impl State {
    fn pending(&self) -> MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn warn(&self, warning: McpWarning) {
        let mut w = self.warnings.lock().unwrap_or_else(|p| p.into_inner());
        if w.len() >= MAX_WARNINGS {
            w.remove(0);
        }
        w.push(warning);
    }
}

/// Połączenie klienta.
pub struct Connection {
    writer: Arc<tokio::sync::Mutex<BoxWriter>>,
    state: Arc<State>,
    next_id: AtomicI64,
    reader_task: tokio::task::JoinHandle<()>,
}

impl Connection {
    /// Uruchamia czytnik odpowiedzi nad strumieniem.
    pub fn new<R, W>(reader: R, writer: W) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let state = Arc::new(State {
            pending: Mutex::new(HashMap::new()),
            warnings: Mutex::new(Vec::new()),
            closed: AtomicBool::new(false),
            tools_changed: AtomicBool::new(false),
        });
        let writer: Arc<tokio::sync::Mutex<BoxWriter>> =
            Arc::new(tokio::sync::Mutex::new(Box::new(writer)));
        let reader_task = tokio::spawn(read_loop(
            BufReader::new(reader),
            state.clone(),
            writer.clone(),
        ));
        Self {
            // Czytnik trzyma kopię pisarza do odpowiedzi na żądania serwera (`ping`).
            writer,
            state,
            next_id: AtomicI64::new(1),
            reader_task,
        }
    }

    /// Wysyła żądanie i czeka na odpowiedź.
    pub async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, McpError> {
        if self.state.closed.load(Ordering::SeqCst) {
            return Err(McpError::Closed);
        }
        let id = RequestId::Number(self.next_id.fetch_add(1, Ordering::SeqCst));
        let (tx, rx) = oneshot::channel();
        self.state.pending().insert(id.clone(), tx);
        let line = Message::request(id.clone(), method, Some(params)).to_line(Dialect::Strict);
        if let Err(e) = self.send_line(&line).await {
            self.state.pending().remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(err))) => Err(McpError::Rpc {
                code: err.code,
                message: err.message,
            }),
            Ok(Err(_)) => Err(McpError::Closed),
            Err(_) => {
                self.state.pending().remove(&id);
                let cancel = Message::notification(
                    methods::CANCELLED,
                    Some(json!({"requestId": id_value(&id), "reason": "timeout"})),
                );
                let _ = self.send_line(&cancel.to_line(Dialect::Strict)).await;
                Err(McpError::Timeout {
                    method: method.to_owned(),
                })
            }
        }
    }

    /// Wysyła powiadomienie.
    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpError> {
        self.send_line(&Message::notification(method, params).to_line(Dialect::Strict))
            .await
    }

    async fn send_line(&self, line: &str) -> Result<(), McpError> {
        let mut w = self.writer.lock().await;
        write_line(&mut **w, line).await
    }

    /// Czy serwer zgłosił zmianę listy narzędzi (zeruje flagę).
    pub fn take_tools_changed(&self) -> bool {
        self.state.tools_changed.swap(false, Ordering::SeqCst)
    }

    /// Ostrzeżenia (opróżnia bufor).
    pub fn take_warnings(&self) -> Vec<McpWarning> {
        std::mem::take(
            &mut *self
                .state
                .warnings
                .lock()
                .unwrap_or_else(|p| p.into_inner()),
        )
    }

    /// Dodaje ostrzeżenie.
    pub fn warn(&self, warning: McpWarning) {
        self.state.warn(warning);
    }

    /// Zamyka połączenie (zapis) i czytnik.
    pub async fn close(&self) {
        let mut w = self.writer.lock().await;
        let _ = w.shutdown().await;
        // Zamknięcie potoku (np. stdin procesu serwera) wymaga porzucenia pisarza.
        *w = Box::new(tokio::io::sink());
        self.reader_task.abort();
        self.state.closed.store(true, Ordering::SeqCst);
        self.state.pending().clear();
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.reader_task.abort();
    }
}

fn id_value(id: &RequestId) -> Value {
    match id {
        RequestId::Number(n) => json!(n),
        RequestId::Text(s) => json!(s),
    }
}

async fn write_line(w: &mut (dyn AsyncWrite + Send + Unpin), line: &str) -> Result<(), McpError> {
    let mut buf = Vec::with_capacity(line.len() + 1);
    buf.extend_from_slice(line.as_bytes());
    buf.push(b'\n');
    w.write_all(&buf)
        .await
        .map_err(|e| McpError::Transport(e.to_string()))?;
    w.flush()
        .await
        .map_err(|e| McpError::Transport(e.to_string()))
}

async fn read_loop<R: AsyncRead + Unpin>(
    mut reader: BufReader<R>,
    state: Arc<State>,
    writer: Arc<tokio::sync::Mutex<BoxWriter>>,
) {
    loop {
        let line = match read_line(&mut reader, DEFAULT_MAX_LINE_BYTES).await {
            Ok(Some(Line::Text(line))) => line,
            Ok(Some(Line::TooLong(bytes))) => {
                state.warn(McpWarning::MalformedLine { bytes });
                continue;
            }
            Ok(None) | Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        match parse_line(&line, Dialect::Strict) {
            Ok(Message::Response { id, outcome }) => {
                if let Some(tx) = state.pending().remove(&id) {
                    let _ = tx.send(outcome);
                }
            }
            Ok(Message::Request { id, method, .. }) => {
                let reply = if method == methods::PING {
                    Message::result(id, json!({}))
                } else {
                    // Alfa nie deklaruje sampling/roots/elicitation — odrzucamy (bezpieczniej).
                    state.warn(McpWarning::RejectedServerRequest {
                        method: method.clone(),
                    });
                    Message::error(id, RpcError::method_not_found(&method))
                };
                let mut w = writer.lock().await;
                let _ = write_line(&mut **w, &reply.to_line(Dialect::Strict)).await;
            }
            Ok(Message::Notification { method, params }) => match method.as_str() {
                methods::TOOLS_LIST_CHANGED => {
                    state.tools_changed.store(true, Ordering::SeqCst);
                    state.warn(McpWarning::ToolListChanged);
                }
                methods::MESSAGE => {
                    let params = params.unwrap_or(Value::Null);
                    let level = params["level"].as_str().unwrap_or("info").to_owned();
                    let mut data = params.get("data").cloned().unwrap_or(Value::Null);
                    if data.to_string().len() > 2000 {
                        data = Value::String("(skrócono)".into());
                    }
                    state.warn(McpWarning::ServerLog { level, data });
                }
                _ => {}
            },
            Err(_) => state.warn(McpWarning::MalformedLine { bytes: line.len() }),
        }
    }
    state.closed.store(true, Ordering::SeqCst);
    // Oczekujące żądania dostają `Closed` (porzucony nadawca).
    state.pending().clear();
}
