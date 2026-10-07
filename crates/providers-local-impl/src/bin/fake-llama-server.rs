//! Fałszywy `llama-server` do testów (bez modelu i bez GPU): HTTP/1.1 na `--host:--port`,
//! `/health` bez klucza, `/v1/*` wymagają `Authorization: Bearer <--api-key>`, strumień
//! Chat Completions (SSE) wg scenariusza. Nigdy nie wiąże się z adresem innym niż `127.0.0.1`.
//!
//! Sterowanie (zmienne środowiskowe ustawiane przez testy):
//! - `FAKE_LLAMA_ARGS_FILE` — dopisz argumenty (po jednym w linii) do pliku;
//! - `FAKE_LLAMA_LOG` — dopisz ciało każdego żądania czatu (jedna linia JSON) do pliku;
//! - `FAKE_LLAMA_MODE` — `ok` | `crash-on-start` | `fail-gpu` (wyjście, gdy `-ngl` ≠ 0) |
//!   `slow-start` (503 „loading" przez 300 ms);
//! - `FAKE_LLAMA_SCENARIO` — JSON: `{"kind":"text","chunks":[..]}` | `tool` | `http_error` |
//!   `refusal` | `max_tokens` | `stall` | `slow`; brak = echo ostatniej wiadomości.
//!   Tekst `CRASH` w ostatniej wiadomości kończy proces w trakcie odpowiedzi.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct Cfg {
    key: String,
    alias: String,
    mode: String,
    scenario: Option<Value>,
    log: Option<String>,
    born: Instant,
    last_request: AtomicU64,
    log_lock: Mutex<()>,
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn append(path: &str, text: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{text}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Ok(path) = std::env::var("FAKE_LLAMA_ARGS_FILE") {
        append(&path, &format!("{}\n---", args.join("\n")));
    }
    let mode = std::env::var("FAKE_LLAMA_MODE").unwrap_or_else(|_| "ok".into());
    let host = arg(&args, "--host").unwrap_or_default();
    if host != "127.0.0.1" {
        std::process::exit(2);
    }
    let gpu = arg(&args, "-ngl").is_some_and(|n| n != "0");
    if mode == "crash-on-start" || (mode == "fail-gpu" && gpu) {
        std::process::exit(4);
    }
    let port = arg(&args, "--port").unwrap_or_default();
    let Ok(listener) = TcpListener::bind(format!("127.0.0.1:{port}")) else {
        std::process::exit(5);
    };
    let cfg = Arc::new(Cfg {
        key: arg(&args, "--api-key").unwrap_or_default(),
        alias: arg(&args, "--alias").unwrap_or_else(|| "model".into()),
        mode,
        scenario: std::env::var("FAKE_LLAMA_SCENARIO")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok()),
        log: std::env::var("FAKE_LLAMA_LOG").ok(),
        born: Instant::now(),
        last_request: AtomicU64::new(0),
        log_lock: Mutex::new(()),
    });
    // Strażnik: bez żądań przez 120 s proces kończy się sam (brak sierot po testach).
    let watchdog = Arc::clone(&cfg);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let last = watchdog.last_request.load(Ordering::SeqCst);
            if watchdog.born.elapsed().as_secs().saturating_sub(last) > 120 {
                std::process::exit(0);
            }
        }
    });
    for conn in listener.incoming().flatten() {
        let cfg = Arc::clone(&cfg);
        std::thread::spawn(move || handle(conn, &cfg));
    }
}

struct Request {
    method: String,
    path: String,
    auth: Option<String>,
    body: String,
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_owned();
    let path = parts.next()?.to_owned();
    let mut len = 0usize;
    let mut auth = None;
    loop {
        let mut h = String::new();
        reader.read_line(&mut h).ok()?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            let v = v.trim();
            match k.to_ascii_lowercase().as_str() {
                "content-length" => len = v.parse().unwrap_or(0),
                "authorization" => auth = Some(v.to_owned()),
                _ => {}
            }
        }
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        path,
        auth,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn respond(stream: &mut TcpStream, status: u16, extra: &str, body: &str) {
    let head = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
}

fn error(stream: &mut TcpStream, status: u16, extra: &str, kind: &str) {
    let body = json!({"error": {"code": status, "message": format!("fake {kind}"), "type": kind}});
    respond(stream, status, extra, &body.to_string());
}

fn handle(mut stream: TcpStream, cfg: &Cfg) {
    let Some(req) = read_request(&stream) else {
        return;
    };
    cfg.last_request
        .store(cfg.born.elapsed().as_secs(), Ordering::SeqCst);
    if req.path == "/health" {
        if cfg.mode == "slow-start" && cfg.born.elapsed() < Duration::from_millis(300) {
            return error(&mut stream, 503, "", "unavailable_error");
        }
        return respond(&mut stream, 200, "", r#"{"status":"ok"}"#);
    }
    if req.auth.as_deref() != Some(&format!("Bearer {}", cfg.key)) {
        return error(&mut stream, 401, "", "authentication_error");
    }
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/v1/models") => {
            let body = json!({"object": "list", "data": [{"id": cfg.alias, "object": "model"}]});
            respond(&mut stream, 200, "", &body.to_string());
        }
        ("POST", "/v1/chat/completions") => chat(&mut stream, cfg, &req.body),
        _ => error(&mut stream, 404, "", "not_found_error"),
    }
}

fn chunk(delta: &Value, finish: Option<&str>) -> String {
    let body = json!({"id": "chatcmpl-fake", "object": "chat.completion.chunk", "model": "fake",
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]});
    format!("data: {body}\n\n")
}

fn usage() -> String {
    let body = json!({"id": "chatcmpl-fake", "object": "chat.completion.chunk", "model": "fake",
        "choices": [], "usage": {"prompt_tokens": 7, "completion_tokens": 3, "total_tokens": 10}});
    format!("data: {body}\n\ndata: [DONE]\n\n")
}

fn last_user_text(body: &Value) -> String {
    body["messages"]
        .as_array()
        .and_then(|m| m.last())
        .map(|m| m["content"].as_str().unwrap_or_default().to_owned())
        .unwrap_or_default()
}

fn chat(stream: &mut TcpStream, cfg: &Cfg, raw: &str) {
    if let Some(path) = &cfg.log {
        let _guard = cfg.log_lock.lock();
        append(path, raw);
    }
    let body: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
    let scenario = cfg
        .scenario
        .clone()
        .unwrap_or_else(|| json!({"kind": "text", "chunks": ["Echo: ", last_user_text(&body)]}));
    let kind = scenario["kind"].as_str().unwrap_or("text");
    if kind == "http_error" {
        let status = u16::try_from(scenario["status"].as_u64().unwrap_or(500)).unwrap_or(500);
        let extra = scenario["retry_after"]
            .as_u64()
            .map(|s| format!("retry-after: {s}\r\n"))
            .unwrap_or_default();
        let t = match status {
            401 => "authentication_error",
            429 => "rate_limit_exceeded",
            400 => "invalid_request_error",
            _ => "server_error",
        };
        return error(stream, status, &extra, t);
    }
    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n";
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let mut send = |s: String| {
        stream
            .write_all(s.as_bytes())
            .and_then(|()| stream.flush())
            .is_ok()
    };
    let texts = |key: &str| -> Vec<String> {
        scenario[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    match kind {
        "stall" => std::thread::sleep(Duration::from_secs(30)),
        "tool" => {
            let args = scenario["arguments"].to_string();
            let (a, b) = args.split_at(args.len() / 2);
            let start = json!({"tool_calls": [{"index": 0, "id": scenario["id"], "type": "function",
                "function": {"name": scenario["name"], "arguments": a}}]});
            let rest = json!({"tool_calls": [{"index": 0, "function": {"arguments": b}}]});
            let _ = send(chunk(&start, None))
                && send(chunk(&rest, None))
                && send(chunk(&json!({}), Some("tool_calls")))
                && send(usage());
        }
        "refusal" => {
            let _ =
                send(chunk(&json!({"role": "assistant"}), Some("content_filter"))) && send(usage());
        }
        "max_tokens" => {
            let _ = send(chunk(&json!({"content": scenario["text"]}), None))
                && send(chunk(&json!({}), Some("length")))
                && send(usage());
        }
        _ => {
            let chunks = texts("chunks");
            let gap = Duration::from_millis(scenario["interval_ms"].as_u64().unwrap_or(0));
            let crash = last_user_text(&body).contains("CRASH");
            for (i, c) in chunks.iter().enumerate() {
                if i > 0 && !gap.is_zero() {
                    std::thread::sleep(gap);
                }
                if !send(chunk(&json!({"content": c}), None)) {
                    return;
                }
                if crash {
                    std::process::exit(9);
                }
            }
            let _ = send(chunk(&json!({}), Some("stop"))) && send(usage());
        }
    }
}
