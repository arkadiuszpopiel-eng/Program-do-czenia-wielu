//! Atrapa zewnętrznego serwera MCP jako osobny proces (stdio) — testy uruchamiania serwerów
//! podłączanych przez użytkownika: `echo`, `env_names` (nazwy zmiennych środowiska, bez wartości)
//! i `secret_len` (długość `ALFA_FAKE_SECRET` — sprawdza przekazanie sekretu bez jego ujawnienia).
//! `ALFA_FAKE_MCP_DESCRIPTION` zmienia opis `echo` (podmiana opisu po zgodzie);
//! `ALFA_FAKE_MCP_FAIL` — nieudany start z komunikatem na stderr.

use async_trait::async_trait;
use mcp_contract::{
    CallToolResult, Implementation, ServerSession, Tool, ToolCallError, ToolHandler,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct Handler;

fn tool(name: &str, description: &str, schema: Value) -> Tool {
    Tool {
        name: name.to_owned(),
        title: None,
        description: Some(description.to_owned()),
        input_schema: schema,
        output_schema: None,
        annotations: None,
    }
}

#[async_trait]
impl ToolHandler for Handler {
    fn tools(&self) -> Vec<Tool> {
        let echo = std::env::var("ALFA_FAKE_MCP_DESCRIPTION")
            .unwrap_or_else(|_| "Zwraca podany tekst.".to_owned());
        vec![
            tool(
                "echo",
                &echo,
                json!({"type": "object", "properties": {"text": {"type": "string"}},
                       "required": ["text"]}),
            ),
            tool(
                "env_names",
                "Nazwy zmiennych środowiska procesu.",
                json!({"type": "object", "properties": {}}),
            ),
            tool(
                "secret_len",
                "Długość zmiennej ALFA_FAKE_SECRET.",
                json!({"type": "object", "properties": {}}),
            ),
        ]
    }

    async fn call(&self, name: &str, arguments: Value) -> Result<CallToolResult, ToolCallError> {
        match name {
            "echo" => Ok(CallToolResult::text(
                arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )),
            "env_names" => {
                let mut names: Vec<String> = std::env::vars_os()
                    .map(|(k, _)| k.to_string_lossy().to_uppercase())
                    .collect();
                names.sort();
                Ok(CallToolResult::text(names.join(",")))
            }
            "secret_len" => Ok(CallToolResult::text(
                std::env::var("ALFA_FAKE_SECRET")
                    .map(|s| s.len())
                    .unwrap_or(0)
                    .to_string(),
            )),
            other => Err(ToolCallError::Unknown(other.to_owned())),
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Nieudany start (jak `npm ERR! 404` przy `npx`): komunikat na stderr i wyjście bez rozmowy.
    if let Ok(reason) = std::env::var("ALFA_FAKE_MCP_FAIL") {
        eprintln!("npm ERR! 404 {reason}");
        std::process::exit(3);
    }
    let session = ServerSession::new(
        Handler,
        Implementation {
            name: "alfa-fake-mcp-server".into(),
            title: None,
            version: env!("CARGO_PKG_VERSION").into(),
        },
    );
    let mut out = tokio::io::stdout();
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(reply) = session.handle_line(&line).await {
            if out
                .write_all(format!("{reply}\n").as_bytes())
                .await
                .is_err()
            {
                break;
            }
            let _ = out.flush().await;
        }
    }
}
