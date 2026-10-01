//! Fałszywy zewnętrzny serwer MCP w procesie: narzędzia sterowane z testu (zmiana opisu po
//! zatwierdzeniu, złośliwy opis), rejestr wywołań, opcjonalne `notifications/tools/list_changed`.

use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use mcp_contract::jsonrpc::{Dialect, Message};
use mcp_contract::protocol::methods;
use mcp_contract::{
    CallToolResult, Implementation, ServerSession, Tool, ToolCallError, ToolHandler,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

#[derive(Default)]
struct State {
    tools: Vec<Tool>,
    calls: Vec<(String, Value)>,
}

/// Uchwyt sterujący serwerem.
#[derive(Clone, Default)]
pub struct FakeMcpServer {
    state: Arc<Mutex<State>>,
    notify: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<String>>>>,
}

/// Prosta definicja narzędzia (obiekt bez wymaganych pól).
pub fn simple_tool(name: &str, description: &str) -> Tool {
    Tool {
        name: name.to_owned(),
        title: None,
        description: Some(description.to_owned()),
        input_schema: json!({"type": "object", "properties": {}}),
        output_schema: None,
        annotations: None,
    }
}

impl FakeMcpServer {
    /// Serwer z narzędziami.
    pub fn new(tools: Vec<Tool>) -> Self {
        let server = Self::default();
        server.lock().tools = tools;
        server
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Zmienia opis narzędzia (rug pull); `notify` — czy wysłać `tools/list_changed`.
    pub fn set_description(&self, tool: &str, description: &str, notify: bool) {
        for t in self.lock().tools.iter_mut().filter(|t| t.name == tool) {
            t.description = Some(description.to_owned());
        }
        if notify {
            let line =
                Message::notification(methods::TOOLS_LIST_CHANGED, None).to_line(Dialect::Strict);
            if let Some(tx) = self
                .notify
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
            {
                let _ = tx.send(line);
            }
        }
    }

    /// Wywołania narzędzi (nazwa, argumenty).
    pub fn calls(&self) -> Vec<(String, Value)> {
        self.lock().calls.clone()
    }

    /// Uruchamia sesję w procesie; zwraca strumień klienta (czytanie, pisanie).
    pub fn serve_duplex(
        &self,
    ) -> (
        tokio::io::ReadHalf<DuplexStream>,
        tokio::io::WriteHalf<DuplexStream>,
    ) {
        let (client, server) = tokio::io::duplex(1 << 20);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        *self.notify.lock().unwrap_or_else(|p| p.into_inner()) = Some(tx.clone());
        let session = ServerSession::new(
            Handler(self.clone()),
            Implementation {
                name: "fake-external".into(),
                title: None,
                version: "0.0.0".into(),
            },
        );
        let (r, mut w) = tokio::io::split(server);
        tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                if w.write_all(format!("{line}\n").as_bytes()).await.is_err() {
                    break;
                }
            }
        });
        tokio::spawn(async move {
            let mut lines = BufReader::new(r).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(out) = session.handle_line(&line).await
                    && tx.send(out).is_err()
                {
                    break;
                }
            }
        });
        tokio::io::split(client)
    }
}

struct Handler(FakeMcpServer);

#[async_trait]
impl ToolHandler for Handler {
    fn tools(&self) -> Vec<Tool> {
        self.0.lock().tools.clone()
    }

    async fn call(&self, name: &str, arguments: Value) -> Result<CallToolResult, ToolCallError> {
        let mut st = self.0.lock();
        if !st.tools.iter().any(|t| t.name == name) {
            return Err(ToolCallError::Unknown(name.to_owned()));
        }
        st.calls.push((name.to_owned(), arguments.clone()));
        Ok(CallToolResult::structured(
            json!({"tool": name, "echo": arguments}),
        ))
    }
}
