//! Atrapa hosta MCP mostów: prawdziwy kanał lokalny (gniazdo Unix / named pipe — nigdy TCP),
//! ten sam protokół powitania i MCP co `mcp-impl`, ale narzędzia Windows zwracają dane stałe,
//! a zegar TTL jest ręczny. Służy testom mostów (`agent-backends`), które nie mogą zależeć
//! od `mcp-impl`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use mcp_contract::bridge::{ENV_ENDPOINT, ENV_TOKEN, MAX_HELLO_BYTES};
use mcp_contract::{
    ALFA_SERVER_NAME, AlfaTool, ApprovalRouter, BridgeMcpHost, BridgeRegistration, BridgeScope,
    CallToolResult, LocalEndpoint, McpError, McpServerLaunch, PermissionPromptRequest, ProxyHello,
    RegistrationId, ServerSession, SessionToken, TokenTable, Tool, ToolCallError, ToolHandler,
    alfa::alfa_server_info,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

#[derive(Clone)]
struct Registration {
    scope: BridgeScope,
    approvals: Option<Arc<dyn ApprovalRouter>>,
}

#[derive(Default)]
struct State {
    table: TokenTable<Registration>,
    approvals: Vec<PermissionPromptRequest>,
    rejected: u32,
}

struct Shared {
    state: Mutex<State>,
    now_ms: AtomicU64,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Atrapa `BridgeMcpHost`.
pub struct FakeBridgeMcpHost {
    shared: Arc<Shared>,
    endpoint: LocalEndpoint,
    ttl_ms: u64,
    counter: AtomicU64,
    accept: tokio::task::JoinHandle<()>,
    #[cfg(unix)]
    dir: PathBuf,
}

impl FakeBridgeMcpHost {
    /// Uruchamia atrapę z TTL tokenu (ms zegara ręcznego, start = 0).
    pub fn start(ttl_ms: u64) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            now_ms: AtomicU64::new(0),
        });
        let unique = uuid::Uuid::new_v4().simple().to_string();
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            let dir = std::env::temp_dir().join(format!("alfa-mcp-fake-{}", &unique[..12]));
            std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
            let socket = dir.join("mcp.sock");
            let listener = tokio::net::UnixListener::bind(&socket)?;
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
            let s = shared.clone();
            let accept = tokio::spawn(async move {
                while let Ok((stream, _)) = listener.accept().await {
                    tokio::spawn(serve(s.clone(), stream));
                }
            });
            Ok(Self {
                shared,
                endpoint: LocalEndpoint::UnixSocket(socket),
                ttl_ms,
                counter: AtomicU64::new(0),
                accept,
                dir,
            })
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;
            let name = format!(r"\\.\pipe\alfa-mcp-fake-{unique}");
            let mut next = ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(&name)?;
            let s = shared.clone();
            let pipe_name = name.clone();
            let accept = tokio::spawn(async move {
                while next.connect().await.is_ok() {
                    let Ok(fresh) = ServerOptions::new().create(&pipe_name) else {
                        break;
                    };
                    let connected = std::mem::replace(&mut next, fresh);
                    tokio::spawn(serve(s.clone(), connected));
                }
            });
            Ok(Self {
                shared,
                endpoint: LocalEndpoint::NamedPipe(name),
                ttl_ms,
                counter: AtomicU64::new(0),
                accept,
            })
        }
    }

    /// Adres kanału.
    pub fn endpoint(&self) -> &LocalEndpoint {
        &self.endpoint
    }

    /// Przesuwa zegar TTL.
    pub fn advance_ms(&self, ms: u64) {
        self.shared.now_ms.fetch_add(ms, Ordering::SeqCst);
    }

    /// Prośby `approve`, które dotarły do hosta.
    pub fn approvals(&self) -> Vec<PermissionPromptRequest> {
        self.shared.lock().approvals.clone()
    }

    /// Liczba odrzuconych połączeń (zły/wygasły/unieważniony token).
    pub fn rejected(&self) -> u32 {
        self.shared.lock().rejected
    }
}

impl Drop for FakeBridgeMcpHost {
    fn drop(&mut self) {
        self.accept.abort();
        #[cfg(unix)]
        {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

#[async_trait]
impl BridgeMcpHost for FakeBridgeMcpHost {
    async fn register(
        &self,
        scope: BridgeScope,
        approvals: Option<Arc<dyn ApprovalRouter>>,
    ) -> Result<BridgeRegistration, McpError> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let token = format!("{}{}", uuid::Uuid::new_v4().simple(), n);
        let id = RegistrationId(format!("fake-{n}"));
        let expires_at_ms = self.shared.now_ms.load(Ordering::SeqCst) + self.ttl_ms;
        self.shared.lock().table.insert(
            id.clone(),
            SessionToken::new(token.clone()),
            expires_at_ms,
            Registration { scope, approvals },
        );
        Ok(BridgeRegistration {
            id,
            launch: McpServerLaunch {
                name: ALFA_SERVER_NAME.to_owned(),
                command: PathBuf::from(mcp_contract::bridge::PROXY_PROGRAM),
                args: Vec::new(),
                env: BTreeMap::from([
                    (ENV_ENDPOINT.to_owned(), self.endpoint.to_env_value()),
                    (ENV_TOKEN.to_owned(), token),
                ]),
            },
            expires_at_ms,
        })
    }

    async fn revoke(&self, id: &RegistrationId) -> Result<(), McpError> {
        if self.shared.lock().table.revoke(id) {
            Ok(())
        } else {
            Err(McpError::Unauthorized("nieznana rejestracja".into()))
        }
    }
}

struct Tools {
    visible: Vec<AlfaTool>,
    approvals: Option<Arc<dyn ApprovalRouter>>,
    shared: Arc<Shared>,
}

#[async_trait]
impl ToolHandler for Tools {
    fn tools(&self) -> Vec<Tool> {
        self.visible.iter().map(|t| t.definition()).collect()
    }

    async fn call(&self, name: &str, arguments: Value) -> Result<CallToolResult, ToolCallError> {
        let tool = AlfaTool::from_name(name)
            .filter(|t| self.visible.contains(t))
            .ok_or_else(|| ToolCallError::Unknown(name.to_owned()))?;
        match (tool, &self.approvals) {
            (AlfaTool::Approve, Some(router)) => {
                let req: PermissionPromptRequest = serde_json::from_value(arguments)
                    .map_err(|e| ToolCallError::InvalidParams(e.to_string()))?;
                self.shared.lock().approvals.push(req.clone());
                let resp = router.permission_prompt(req).await;
                let text = serde_json::to_string(&resp)
                    .map_err(|e| ToolCallError::InvalidParams(e.to_string()))?;
                Ok(CallToolResult::text(text))
            }
            (AlfaTool::ClipboardRead, _) => Ok(CallToolResult::structured(
                json!({"kind": "text", "text": "atrapa"}),
            )),
            (AlfaTool::WindowsList, _) => Ok(CallToolResult::structured(json!({"windows": []}))),
            // v1 (UIA, zrzut, rejestr): dane stałe z oznaczeniem jak w `mcp-impl`.
            (t, _) if t.is_v1() => Ok(CallToolResult::structured(
                json!({"atrapa": t.name(), mcp_contract::UNVERIFIED_FIELD: true}),
            )),
            _ => Ok(CallToolResult::text("ok (atrapa)")),
        }
    }
}

async fn serve<S: AsyncRead + AsyncWrite + Send + Unpin + 'static>(shared: Arc<Shared>, stream: S) {
    let (r, w) = tokio::io::split(stream);
    let mut reader = BufReader::new(r);
    let mut first = String::new();
    let hello = match reader.read_line(&mut first).await {
        Ok(n) if n > 0 && first.len() <= MAX_HELLO_BYTES => ProxyHello::parse(&first),
        _ => None,
    };
    let now = shared.now_ms.load(Ordering::SeqCst);
    let validated = hello.and_then(|h| shared.lock().table.validate(&h.token, now).ok());
    let Some((_, reg)) = validated else {
        shared.lock().rejected += 1;
        return;
    };
    let mut visible: Vec<AlfaTool> = reg
        .scope
        .tools
        .iter()
        .copied()
        .filter(|t| *t != AlfaTool::Approve)
        .collect();
    if reg.approvals.is_some() {
        visible.push(AlfaTool::Approve);
    }
    let tools = Tools {
        visible,
        approvals: reg.approvals,
        shared: shared.clone(),
    };
    let session = Arc::new(ServerSession::new(tools, alfa_server_info()));
    let writer = Arc::new(tokio::sync::Mutex::new(w));
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let (session, writer) = (session.clone(), writer.clone());
        tokio::spawn(async move {
            if let Some(out) = session.handle_line(&line).await {
                let mut w = writer.lock().await;
                let _ = w.write_all(format!("{out}\n").as_bytes()).await;
                let _ = w.flush().await;
            }
        });
    }
}
