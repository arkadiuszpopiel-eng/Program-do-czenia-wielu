//! Wspólne narzędzia fałszywego CLI: wyjście linii, czas, proces-wnuk, połączenie z MCP Alfy.

use std::io::{BufRead, BufReader, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// Wersja zgłaszana przez `--version` (testy przypinają ją albo celowo nie).
pub const FAKE_VERSION: &str = "alfa-fake-agent-cli 9.8.7";

/// Wypisuje jedną linię JSON na stdout i opróżnia bufor.
pub fn emit(value: &Value) {
    raw(&value.to_string());
}

/// Wypisuje dowolną linię (śmieci, bardzo długie linie).
pub fn raw(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(line.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

/// Czas uniksowy w ms.
pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Uruchamia proces-wnuka (ta sama binarka w trybie `--alfa-fake-sleep`); zwraca PID.
pub fn spawn_grandchild() -> u32 {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            std::process::Command::new(exe)
                .arg("--alfa-fake-sleep")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok()
        })
        .map_or(0, |c| c.id())
}

/// Nazwy zmiennych środowiska procesu (posortowane).
pub fn env_names() -> String {
    let mut names: Vec<String> = std::env::vars_os()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.join(",")
}

/// Strona czytająca i pisząca kanału.
type Duplex = (Box<dyn Read>, Box<dyn Write>);

/// Połączenie z kanałem MCP Alfy. Prawdziwe CLI uruchamia `alfa-mcp-proxy` (pompa bajtów);
/// atrapa łączy się z kanałem bezpośrednio tym samym protokołem (powitanie + MCP).
pub struct McpLink {
    reader: BufReader<Box<dyn Read>>,
    writer: Box<dyn Write>,
    next_id: i64,
}

impl McpLink {
    /// Łączy się wg wpisu `mcpServers.alfa` z pliku `--mcp-config`.
    pub fn connect(config_path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(config_path).map_err(|e| format!("mcp-config: {e}"))?;
        let cfg: Value = serde_json::from_str(&text).map_err(|e| format!("mcp-config: {e}"))?;
        let env = &cfg["mcpServers"]["alfa"]["env"];
        let endpoint = env["ALFA_MCP_ENDPOINT"]
            .as_str()
            .ok_or("brak ALFA_MCP_ENDPOINT")?;
        let token = env["ALFA_MCP_TOKEN"]
            .as_str()
            .ok_or("brak ALFA_MCP_TOKEN")?;
        let (reader, writer) = open(endpoint)?;
        let mut link = Self {
            reader: BufReader::new(reader),
            writer,
            next_id: 0,
        };
        link.line(&json!({"type": "alfa-mcp-hello", "version": 1, "token": token}))?;
        let init = link.call(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "fake-claude", "version": "0"}}),
        )?;
        if init.get("result").is_none() {
            return Err(format!("initialize odrzucone: {init}"));
        }
        link.line(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))?;
        Ok(link)
    }

    fn line(&mut self, v: &Value) -> Result<(), String> {
        let mut bytes = v.to_string().into_bytes();
        bytes.push(b'\n');
        self.writer.write_all(&bytes).map_err(|e| e.to_string())?;
        self.writer.flush().map_err(|e| e.to_string())
    }

    /// Żądanie i odpowiedź.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        self.line(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        loop {
            let mut buf = String::new();
            if self.reader.read_line(&mut buf).map_err(|e| e.to_string())? == 0 {
                return Err("kanał MCP zamknięty".into());
            }
            let v: Value = serde_json::from_str(&buf).map_err(|e| e.to_string())?;
            if v["id"] == json!(id) {
                return Ok(v);
            }
        }
    }

    /// Wywołuje `approve` (jak `--permission-prompt-tool`); `Ok(true)` = zgoda.
    pub fn approve(
        &mut self,
        tool: &str,
        input: Value,
        tool_use_id: &str,
    ) -> Result<(bool, String), String> {
        let resp = self.call(
            "tools/call",
            json!({"name": "approve", "arguments": {"tool_name": tool, "input": input, "tool_use_id": tool_use_id}}),
        )?;
        let text = resp["result"]["content"][0]["text"]
            .as_str()
            .ok_or_else(|| format!("zła odpowiedź: {resp}"))?;
        let decision: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let message = decision["message"].as_str().unwrap_or_default().to_owned();
        Ok((decision["behavior"] == "allow", message))
    }
}

#[cfg(unix)]
fn open(endpoint: &str) -> Result<Duplex, String> {
    let path = endpoint
        .strip_prefix("unix:")
        .ok_or("nieobsługiwany kanał")?;
    let stream = std::os::unix::net::UnixStream::connect(path).map_err(|e| e.to_string())?;
    let reader = stream.try_clone().map_err(|e| e.to_string())?;
    Ok((Box::new(reader), Box::new(stream)))
}

#[cfg(windows)]
fn open(endpoint: &str) -> Result<Duplex, String> {
    let name = endpoint
        .strip_prefix("pipe:")
        .ok_or("nieobsługiwany kanał")?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(name)
        .map_err(|e| e.to_string())?;
    let reader = file.try_clone().map_err(|e| e.to_string())?;
    Ok((Box::new(reader), Box::new(file)))
}
