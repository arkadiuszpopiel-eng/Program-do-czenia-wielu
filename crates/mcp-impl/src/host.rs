//! Host serwera MCP Alfy dla mostów: kanał lokalny, rejestracje z tokenem (TTL), obsługa MCP
//! w zakresie rejestracji. Proxy (`alfa-mcp-proxy`) jest jedynym klientem kanału.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use mcp_contract::bridge::{ENV_ENDPOINT, ENV_TOKEN, HELLO_TIMEOUT_MS, MAX_HELLO_BYTES};
use mcp_contract::{
    ALFA_SERVER_NAME, ApprovalRouter, BridgeMcpHost, BridgeRegistration, BridgeScope,
    LocalEndpoint, McpError, McpServerLaunch, ProxyHello, RegistrationId, ServerSession,
    SessionToken, TokenTable, alfa::alfa_server_info,
};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::sync::{Semaphore, watch};

use crate::lines::{DEFAULT_MAX_LINE_BYTES, Line, read_line};
use crate::listener::{BoxedStream, LocalListener};
use crate::tools::{AlfaToolHandler, PlatformPorts};

/// Zegar hosta (ms, monotoniczny); atrapa w testach TTL.
pub trait Clock: Send + Sync {
    /// Bieżący czas w ms.
    fn now_ms(&self) -> u64;
}

/// Zegar monotoniczny od startu hosta.
#[derive(Debug)]
pub struct MonotonicClock(Instant);

impl Default for MonotonicClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}

impl Clock for MonotonicClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Konfiguracja hosta.
#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Ścieżka programu `alfa-mcp-proxy` (wpisywana do konfiguracji MCP mostu).
    pub proxy_program: PathBuf,
    /// Katalog bazowy gniazda Unix (poza Windows); krótka ścieżka (limit ~108 B).
    pub socket_dir: PathBuf,
    /// TTL tokenu (ms) na nawiązanie połączenia.
    pub token_ttl_ms: u64,
    /// Limit równoległych wywołań narzędzi na połączenie.
    pub max_inflight_calls: usize,
}

impl HostConfig {
    /// Domyślne wartości z podanym programem proxy.
    pub fn new(proxy_program: impl Into<PathBuf>) -> Self {
        Self {
            proxy_program: proxy_program.into(),
            socket_dir: std::env::temp_dir(),
            token_ttl_ms: mcp_contract::bridge::DEFAULT_TOKEN_TTL_MS,
            max_inflight_calls: 16,
        }
    }
}

#[derive(Clone)]
struct Registration {
    scope: BridgeScope,
    approvals: Option<Arc<dyn ApprovalRouter>>,
    revoked: watch::Receiver<bool>,
}

struct Shared {
    table: Mutex<TokenTable<Registration>>,
    revokers: Mutex<BTreeMap<RegistrationId, watch::Sender<bool>>>,
    clock: Arc<dyn Clock>,
    ports: PlatformPorts,
    config: HostConfig,
    rejected: Mutex<Vec<String>>,
}

impl Shared {
    fn table(&self) -> MutexGuard<'_, TokenTable<Registration>> {
        self.table.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Host MCP dla mostów (implementuje [`BridgeMcpHost`]).
pub struct LocalMcpHost {
    shared: Arc<Shared>,
    endpoint: LocalEndpoint,
    accept_task: tokio::task::JoinHandle<()>,
}

fn random_token() -> String {
    // Dwa UUID v4 z CSPRNG systemu (getrandom) = 244 losowe bity.
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

impl LocalMcpHost {
    /// Otwiera kanał lokalny i zaczyna przyjmować połączenia.
    pub fn start(
        config: HostConfig,
        ports: PlatformPorts,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, McpError> {
        let unique = uuid::Uuid::new_v4().simple().to_string();
        let mut listener = LocalListener::bind(&config.socket_dir, &unique)
            .map_err(|e| McpError::Transport(format!("nie udało się otworzyć kanału: {e}")))?;
        let endpoint = listener.endpoint().clone();
        let shared = Arc::new(Shared {
            table: Mutex::new(TokenTable::new()),
            revokers: Mutex::new(BTreeMap::new()),
            clock,
            ports,
            config,
            rejected: Mutex::new(Vec::new()),
        });
        let accept_shared = shared.clone();
        let accept_task = tokio::spawn(async move {
            while let Ok(stream) = listener.accept().await {
                tokio::spawn(serve_connection(accept_shared.clone(), stream));
            }
        });
        Ok(Self {
            shared,
            endpoint,
            accept_task,
        })
    }

    /// Adres kanału.
    pub fn endpoint(&self) -> &LocalEndpoint {
        &self.endpoint
    }

    /// Powody odrzuconych połączeń (bez tokenów) — diagnostyka i testy.
    pub fn rejections(&self) -> Vec<String> {
        self.shared
            .rejected
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

impl Drop for LocalMcpHost {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

#[async_trait]
impl BridgeMcpHost for LocalMcpHost {
    async fn register(
        &self,
        scope: BridgeScope,
        approvals: Option<Arc<dyn ApprovalRouter>>,
    ) -> Result<BridgeRegistration, McpError> {
        let token = random_token();
        let id = RegistrationId(uuid::Uuid::new_v4().to_string());
        let now = self.shared.clock.now_ms();
        let expires_at_ms = now.saturating_add(self.shared.config.token_ttl_ms);
        let (tx, rx) = watch::channel(false);
        {
            let mut table = self.shared.table();
            table.purge(now);
            table.insert(
                id.clone(),
                SessionToken::new(token.clone()),
                expires_at_ms,
                Registration {
                    scope,
                    approvals,
                    revoked: rx,
                },
            );
        }
        self.shared
            .revokers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.clone(), tx);
        let env = BTreeMap::from([
            (ENV_ENDPOINT.to_owned(), self.endpoint.to_env_value()),
            (ENV_TOKEN.to_owned(), token),
        ]);
        Ok(BridgeRegistration {
            id,
            launch: McpServerLaunch {
                name: ALFA_SERVER_NAME.to_owned(),
                command: self.shared.config.proxy_program.clone(),
                args: Vec::new(),
                env,
            },
            expires_at_ms,
        })
    }

    async fn revoke(&self, id: &RegistrationId) -> Result<(), McpError> {
        let known = self.shared.table().revoke(id);
        if let Some(tx) = self
            .shared
            .revokers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id)
        {
            let _ = tx.send(true);
        }
        if known {
            Ok(())
        } else {
            Err(McpError::Unauthorized("nieznana rejestracja".into()))
        }
    }
}

fn reject(shared: &Shared, reason: impl Into<String>) {
    let reason = reason.into();
    tracing::warn!(powod = %reason, "odrzucono połączenie z kanałem MCP Alfy");
    shared
        .rejected
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(reason);
}

async fn serve_connection(shared: Arc<Shared>, stream: BoxedStream) {
    let (r, w) = tokio::io::split(stream);
    let mut reader = BufReader::new(r);
    let hello = tokio::time::timeout(
        Duration::from_millis(HELLO_TIMEOUT_MS),
        read_line(&mut reader, MAX_HELLO_BYTES),
    )
    .await;
    let Ok(Ok(Some(Line::Text(line)))) = hello else {
        reject(&shared, "brak powitania w czasie albo za długie");
        return;
    };
    let Some(hello) = ProxyHello::parse(&line) else {
        reject(&shared, "niepoprawne powitanie");
        return;
    };
    let validated = shared.table().validate(&hello.token, shared.clock.now_ms());
    let (_, registration) = match validated {
        Ok(v) => v,
        Err(why) => {
            reject(&shared, why.to_string());
            return;
        }
    };
    let handler = AlfaToolHandler::new(
        &registration.scope.tools,
        shared.ports.clone(),
        registration.approvals.clone(),
    );
    let session = Arc::new(ServerSession::new(handler, alfa_server_info()));
    let writer = Arc::new(tokio::sync::Mutex::new(w));
    let inflight = Arc::new(Semaphore::new(shared.config.max_inflight_calls.max(1)));
    let mut revoked = registration.revoked.clone();
    loop {
        let next = tokio::select! {
            changed = revoked.wait_for(|r| *r) => { let _ = changed; break; }
            line = read_line(&mut reader, DEFAULT_MAX_LINE_BYTES) => line,
        };
        let line = match next {
            Ok(Some(Line::Text(line))) if line.trim().is_empty() => continue,
            Ok(Some(Line::Text(line))) => line,
            Ok(Some(Line::TooLong(n))) => {
                tracing::warn!(bajty = n, "pominięto za długą linię MCP");
                continue;
            }
            Ok(None) | Err(_) => break,
        };
        let session = session.clone();
        let writer = writer.clone();
        if is_tool_call(&line) {
            let Ok(permit) = inflight.clone().acquire_owned().await else {
                break;
            };
            tokio::spawn(async move {
                if let Some(out) = session.handle_line(&line).await {
                    write_line(&writer, &out).await;
                }
                drop(permit);
            });
        } else if let Some(out) = session.handle_line(&line).await {
            write_line(&writer, &out).await;
        }
    }
    let _ = writer.lock().await.shutdown().await;
}

fn is_tool_call(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|v| v.get("method").and_then(|m| m.as_str().map(str::to_owned)))
        .is_some_and(|m| m == mcp_contract::protocol::methods::TOOLS_CALL)
}

async fn write_line<W: tokio::io::AsyncWrite + Unpin>(writer: &tokio::sync::Mutex<W>, line: &str) {
    let mut w = writer.lock().await;
    let mut buf = Vec::with_capacity(line.len() + 1);
    buf.extend_from_slice(line.as_bytes());
    buf.push(b'\n');
    if w.write_all(&buf).await.is_ok() {
        let _ = w.flush().await;
    }
}
