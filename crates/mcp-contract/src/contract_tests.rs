//! Współdzielone testy kontraktowe hosta mostu (feature `contract-tests`): uruchamiane na
//! `mcp-impl` (prawdziwy kanał lokalny) i `mcp-fake`. Strona klienta mówi dokładnie tym
//! protokołem co `alfa-mcp-proxy`: linia powitania z tokenem, potem MCP JSON-RPC po liniach.

use std::future::Future;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::alfa::AlfaTool;
use crate::bridge::{
    ApprovalRouter, BridgeMcpHost, BridgeScope, LocalEndpoint, PermissionPromptRequest,
    PermissionPromptResponse, ProxyHello,
};
use crate::jsonrpc::{INVALID_PARAMS, METHOD_NOT_FOUND};
use crate::protocol::LATEST_PROTOCOL_VERSION;

/// Router zatwierdzeń do testów: zapisuje prośby, zawsze zgadza się albo zawsze odmawia.
#[derive(Debug, Default)]
pub struct RecordingRouter {
    /// Odmawiaj zamiast zgadzać się.
    pub deny: bool,
    seen: Mutex<Vec<PermissionPromptRequest>>,
}

impl RecordingRouter {
    /// Router zgadzający się.
    pub fn allowing() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Prośby, które dotarły.
    pub fn seen(&self) -> Vec<PermissionPromptRequest> {
        self.seen.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

#[async_trait]
impl ApprovalRouter for RecordingRouter {
    async fn permission_prompt(
        &self,
        request: PermissionPromptRequest,
    ) -> PermissionPromptResponse {
        let input = request.input.clone();
        self.seen
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(request);
        if self.deny {
            PermissionPromptResponse::Deny {
                message: "odmowa testowa".into(),
            }
        } else {
            PermissionPromptResponse::Allow {
                updated_input: input,
            }
        }
    }
}

/// Klient testowy po stronie proxy.
pub struct ProxySide<S> {
    reader: BufReader<tokio::io::ReadHalf<S>>,
    writer: tokio::io::WriteHalf<S>,
    next_id: i64,
}

impl<S: AsyncRead + AsyncWrite + Send> ProxySide<S> {
    /// Wysyła powitanie z tokenem.
    pub async fn hello(stream: S, token: &str) -> Self {
        let (r, mut w) = tokio::io::split(stream);
        let line = format!("{}\n", ProxyHello::new(token).to_line());
        // Host może zamknąć połączenie od razu — błąd zapisu nie przerywa testu.
        let _ = w.write_all(line.as_bytes()).await;
        Self {
            reader: BufReader::new(r),
            writer: w,
            next_id: 0,
        }
    }

    /// Wysyła żądanie i czeka na odpowiedź; `None`, gdy host zamknął połączenie.
    pub async fn request(&mut self, method: &str, params: Value) -> Option<Value> {
        self.next_id += 1;
        let line =
            json!({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params});
        self.writer
            .write_all(format!("{line}\n").as_bytes())
            .await
            .ok()?;
        let mut buf = String::new();
        let n = self.reader.read_line(&mut buf).await.ok()?;
        (n > 0).then(|| serde_json::from_str(&buf).ok()).flatten()
    }

    /// `initialize` + `notifications/initialized`.
    pub async fn initialize(&mut self) -> Option<Value> {
        let r = self
            .request(
                "initialize",
                json!({"protocolVersion": LATEST_PROTOCOL_VERSION, "capabilities": {},
                       "clientInfo": {"name": "contract-test", "version": "0"}}),
            )
            .await?;
        let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        self.writer
            .write_all(format!("{note}\n").as_bytes())
            .await
            .ok()?;
        Some(r)
    }
}

fn endpoint_and_token(reg: &crate::bridge::BridgeRegistration) -> (LocalEndpoint, String) {
    let ep = reg
        .launch
        .endpoint()
        .unwrap_or_else(|| panic!("rejestracja bez kanału lokalnego"));
    let token = reg
        .launch
        .token()
        .unwrap_or_else(|| panic!("rejestracja bez tokenu"))
        .to_owned();
    (ep, token)
}

/// Uruchamia cały zestaw. `connect` otwiera połączenie z kanałem lokalnym (jak proxy).
pub async fn run_all<H, C, Fut, S>(host: &H, connect: C)
where
    H: BridgeMcpHost,
    C: Fn(LocalEndpoint) -> Fut,
    Fut: Future<Output = std::io::Result<S>>,
    S: AsyncRead + AsyncWrite + Send,
{
    // 1. Zakres + `approve` → router.
    let router = RecordingRouter::allowing();
    let reg = host
        .register(BridgeScope::windows_v0("zadanie-1"), Some(router.clone()))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(reg.launch.to_mcp_config()["mcpServers"]["alfa"].is_object());
    let (ep, token) = endpoint_and_token(&reg);
    let stream = connect(ep.clone()).await.unwrap_or_else(|e| panic!("{e}"));
    let mut side = ProxySide::hello(stream, &token).await;
    let init = side
        .initialize()
        .await
        .unwrap_or_else(|| panic!("brak initialize"));
    assert_eq!(init["result"]["serverInfo"]["name"], "alfa");
    let list = side
        .request("tools/list", json!({}))
        .await
        .unwrap_or_default();
    let mut names: Vec<String> = list["result"]["tools"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| t["name"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let mut expected: Vec<String> = AlfaTool::ALL.iter().map(|t| t.name().to_owned()).collect();
    expected.sort();
    assert_eq!(names, expected);
    let call = side
        .request(
            "tools/call",
            json!({"name": "approve", "arguments": {"tool_name": "Write",
                   "input": {"file_path": "a.txt"}, "tool_use_id": "toolu_1"}}),
        )
        .await
        .unwrap_or_default();
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    let decision: Value = serde_json::from_str(text).unwrap_or_default();
    assert_eq!(decision["behavior"], "allow");
    assert_eq!(router.seen().len(), 1);
    assert_eq!(router.seen()[0].tool_name, "Write");
    let unknown = side
        .request("resources/list", json!({}))
        .await
        .unwrap_or_default();
    assert_eq!(unknown["error"]["code"], METHOD_NOT_FOUND);
    let fs = side
        .request("tools/call", json!({"name": "fs_read", "arguments": {}}))
        .await
        .unwrap_or_default();
    assert_eq!(fs["error"]["code"], INVALID_PARAMS);

    // 2. Zły token → połączenie zamknięte bez odpowiedzi MCP.
    let stream = connect(ep.clone()).await.unwrap_or_else(|e| panic!("{e}"));
    let mut bad = ProxySide::hello(stream, "zly-token").await;
    assert!(bad.initialize().await.is_none(), "zły token przyjęty");

    // 3. Unieważnienie → nowy klient z tym tokenem odrzucony.
    host.revoke(&reg.id).await.unwrap_or_else(|e| panic!("{e}"));
    let stream = connect(ep).await.unwrap_or_else(|e| panic!("{e}"));
    let mut revoked = ProxySide::hello(stream, &token).await;
    assert!(
        revoked.initialize().await.is_none(),
        "unieważniony token przyjęty"
    );

    // 4. Bez routera zatwierdzeń `approve` nie istnieje; zakres ogranicza narzędzia.
    let scope = BridgeScope {
        label: "zadanie-2".into(),
        tools: [AlfaTool::WindowsList].into_iter().collect(),
    };
    let reg2 = host
        .register(scope, None)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let (ep2, token2) = endpoint_and_token(&reg2);
    assert_ne!(token2, token, "tokeny rejestracji muszą być różne");
    let stream = connect(ep2).await.unwrap_or_else(|e| panic!("{e}"));
    let mut side2 = ProxySide::hello(stream, &token2).await;
    side2
        .initialize()
        .await
        .unwrap_or_else(|| panic!("brak initialize"));
    let list2 = side2
        .request("tools/list", json!({}))
        .await
        .unwrap_or_default();
    assert_eq!(list2["result"]["tools"].as_array().map(Vec::len), Some(1));
    let approve = side2
        .request(
            "tools/call",
            json!({"name": "approve", "arguments": {"tool_name": "x", "input": {}}}),
        )
        .await
        .unwrap_or_default();
    assert_eq!(approve["error"]["code"], INVALID_PARAMS);
    host.revoke(&reg2.id)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
}
