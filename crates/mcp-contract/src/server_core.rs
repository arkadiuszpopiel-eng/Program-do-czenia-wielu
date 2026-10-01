//! Obsługa protokołu po stronie serwera MCP (czysta logika wspólna `-impl` i `-fake`):
//! `initialize` z negocjacją wersji, `ping`, `tools/list`, `tools/call`; nieznane metody → -32601;
//! żądania przed `initialize` → -32002. Transport (linie stdio/kanał lokalny) jest poza tym modułem.

use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::jsonrpc::{
    Dialect, INVALID_PARAMS, Message, NOT_INITIALIZED, RpcError, UNAUTHORIZED,
    error_line_without_id, parse_line,
};
use crate::protocol::{
    CallToolParams, CallToolResult, Implementation, InitializeParams, LATEST_PROTOCOL_VERSION,
    Tool, is_supported_version, methods,
};

/// Błąd wywołania narzędzia na poziomie protokołu (błędy wykonania to `CallToolResult::failure`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCallError {
    /// Nieznane narzędzie (lub spoza zakresu rejestracji — nie ujawniamy różnicy).
    Unknown(String),
    /// Niepoprawne argumenty.
    InvalidParams(String),
    /// Odmowa polityki (np. okno chronione).
    Unauthorized(String),
}

/// Zestaw narzędzi serwera.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Narzędzia widoczne dla tej sesji.
    fn tools(&self) -> Vec<Tool>;

    /// Wywołanie narzędzia.
    async fn call(&self, name: &str, arguments: Value) -> Result<CallToolResult, ToolCallError>;
}

/// Stan jednej sesji MCP po stronie serwera.
/// Metody można obsługiwać współbieżnie (`&self`): host uruchamia `tools/call` w osobnych
/// zadaniach, żeby czekające `approve` nie blokowało innych żądań połączenia.
pub struct ServerSession<H> {
    handler: H,
    info: Implementation,
    protocol_version: Mutex<Option<String>>,
}

impl<H: ToolHandler> ServerSession<H> {
    /// Nowa sesja.
    pub fn new(handler: H, info: Implementation) -> Self {
        Self {
            handler,
            info,
            protocol_version: Mutex::new(None),
        }
    }

    /// Wynegocjowana wersja protokołu.
    pub fn protocol_version(&self) -> Option<String> {
        self.version_lock().clone()
    }

    fn version_lock(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.protocol_version
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    /// Obsługuje jedną linię; zwraca linię odpowiedzi (albo `None` dla powiadomień i odpowiedzi).
    pub async fn handle_line(&self, line: &str) -> Option<String> {
        match parse_line(line, Dialect::Strict) {
            Ok(msg) => self.handle(msg).await.map(|m| m.to_line(Dialect::Strict)),
            Err(failure) => Some(match failure.id {
                Some(id) => Message::error(id, failure.error).to_line(Dialect::Strict),
                None => error_line_without_id(&failure.error, Dialect::Strict),
            }),
        }
    }

    /// Obsługuje wiadomość.
    pub async fn handle(&self, msg: Message) -> Option<Message> {
        match msg {
            Message::Request { id, method, params } => {
                let outcome = self.request(&method, params).await;
                Some(Message::Response { id, outcome })
            }
            // `notifications/initialized`, `notifications/cancelled` i inne — bez odpowiedzi.
            Message::Notification { .. } => None,
            // Serwer Alfy nie wysyła żądań do klienta — odpowiedzi ignorujemy.
            Message::Response { .. } => None,
        }
    }

    async fn request(&self, method: &str, params: Option<Value>) -> Result<Value, RpcError> {
        if method == methods::INITIALIZE {
            return self.initialize(params);
        }
        if method == methods::PING {
            return Ok(json!({}));
        }
        if self.version_lock().is_none() {
            return Err(RpcError::new(
                NOT_INITIALIZED,
                "sesja nie została zainicjalizowana",
            ));
        }
        match method {
            methods::TOOLS_LIST => Ok(json!({"tools": self.handler.tools()})),
            methods::TOOLS_CALL => self.call(params).await,
            other => Err(RpcError::method_not_found(other)),
        }
    }

    fn initialize(&self, params: Option<Value>) -> Result<Value, RpcError> {
        let params: InitializeParams = serde_json::from_value(params.unwrap_or(Value::Null))
            .map_err(|e| RpcError::invalid_params(format!("niepoprawne `initialize`: {e}")))?;
        let version = if is_supported_version(&params.protocol_version) {
            params.protocol_version
        } else {
            LATEST_PROTOCOL_VERSION.to_owned()
        };
        *self.version_lock() = Some(version.clone());
        Ok(json!({
            "protocolVersion": version,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": self.info,
        }))
    }

    async fn call(&self, params: Option<Value>) -> Result<Value, RpcError> {
        let params: CallToolParams = serde_json::from_value(params.unwrap_or(Value::Null))
            .map_err(|e| RpcError::invalid_params(format!("niepoprawne `tools/call`: {e}")))?;
        let arguments = params.arguments.unwrap_or_else(|| json!({}));
        if !arguments.is_object() {
            return Err(RpcError::invalid_params("`arguments` musi być obiektem"));
        }
        match self.handler.call(&params.name, arguments).await {
            Ok(result) => serde_json::to_value(result)
                .map_err(|e| RpcError::new(crate::jsonrpc::INTERNAL_ERROR, e.to_string())),
            Err(ToolCallError::Unknown(name)) => Err(RpcError::new(
                INVALID_PARAMS,
                format!("nieznane narzędzie `{name}`"),
            )),
            Err(ToolCallError::InvalidParams(m)) => Err(RpcError::invalid_params(m)),
            Err(ToolCallError::Unauthorized(m)) => Err(RpcError::new(UNAUTHORIZED, m)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonrpc::{METHOD_NOT_FOUND, PARSE_ERROR, RequestId};

    struct Echo;

    #[async_trait]
    impl ToolHandler for Echo {
        fn tools(&self) -> Vec<Tool> {
            vec![crate::alfa::AlfaTool::WindowsList.definition()]
        }

        async fn call(
            &self,
            name: &str,
            arguments: Value,
        ) -> Result<CallToolResult, ToolCallError> {
            match name {
                "windows_list" => Ok(CallToolResult::structured(arguments)),
                "bad" => Err(ToolCallError::InvalidParams("zły".into())),
                "deny" => Err(ToolCallError::Unauthorized("nie".into())),
                other => Err(ToolCallError::Unknown(other.into())),
            }
        }
    }

    fn info() -> Implementation {
        crate::alfa::alfa_server_info()
    }

    async fn ask(s: &ServerSession<Echo>, line: &str) -> Value {
        serde_json::from_str(&s.handle_line(line).await.unwrap()).unwrap()
    }

    #[tokio::test]
    async fn lifecycle_and_errors() {
        let s = ServerSession::new(Echo, info());
        let early = ask(&s, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).await;
        assert_eq!(early["error"]["code"], NOT_INITIALIZED);
        let pong = ask(&s, r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#).await;
        assert_eq!(pong["result"], json!({}));
        let init = ask(
            &s,
            r#"{"jsonrpc":"2.0","id":3,"method":"initialize","params":{"protocolVersion":"2099-01-01","capabilities":{},"clientInfo":{"name":"c","version":"1"}}}"#,
        )
        .await;
        assert_eq!(init["result"]["protocolVersion"], LATEST_PROTOCOL_VERSION);
        assert_eq!(
            s.protocol_version().as_deref(),
            Some(LATEST_PROTOCOL_VERSION)
        );
        assert!(
            s.handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .await
                .is_none()
        );
        let list = ask(&s, r#"{"jsonrpc":"2.0","id":4,"method":"tools/list"}"#).await;
        assert_eq!(list["result"]["tools"][0]["name"], "windows_list");
        let call = ask(
            &s,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"windows_list","arguments":{"x":1}}}"#,
        )
        .await;
        assert_eq!(call["result"]["structuredContent"], json!({"x": 1}));
        for (name, code) in [
            ("nope", INVALID_PARAMS),
            ("bad", INVALID_PARAMS),
            ("deny", UNAUTHORIZED),
        ] {
            let line = format!(
                r#"{{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{{"name":"{name}"}}}}"#
            );
            assert_eq!(ask(&s, &line).await["error"]["code"], code);
        }
        let bad_args = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"windows_list","arguments":[1]}}"#;
        assert_eq!(ask(&s, bad_args).await["error"]["code"], INVALID_PARAMS);
        let unknown = ask(&s, r#"{"jsonrpc":"2.0","id":8,"method":"resources/list"}"#).await;
        assert_eq!(unknown["error"]["code"], METHOD_NOT_FOUND);
        let garbage = ask(&s, "nie json").await;
        assert_eq!(garbage["error"]["code"], PARSE_ERROR);
        assert_eq!(garbage["id"], Value::Null);
        let resp = Message::result(RequestId::Number(1), json!({}));
        assert!(s.handle(resp).await.is_none());
        let bad_init = ask(
            &s,
            r#"{"jsonrpc":"2.0","id":9,"method":"initialize","params":{}}"#,
        )
        .await;
        assert_eq!(bad_init["error"]["code"], INVALID_PARAMS);
    }
}
