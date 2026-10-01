//! Atrapa `claude -p` w trybie stream-json (wejście i wyjście) wg dokumentacji Claude Code.

use std::io::BufRead;
use std::time::Duration;

use agent_backends_contract::contract_tests::scenario::{Scenario, permission_result};
use serde_json::{Value, json};

use crate::io::{McpLink, emit, env_names, now_ms, raw, spawn_grandchild};

/// Flagi, bez których atrapa odmawia pracy (most musi je przekazać).
const REQUIRED: [(&str, Option<&str>); 6] = [
    ("--output-format", Some("stream-json")),
    ("--input-format", Some("stream-json")),
    ("--verbose", None),
    ("--permission-prompt-tool", Some("mcp__alfa__approve")),
    ("--permission-mode", Some("default")),
    ("--strict-mcp-config", None),
];

fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn text(t: &str) -> Value {
    json!({"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": t}]}})
}

fn tool_use(id: &str, name: &str, input: Value) -> Value {
    json!({"type": "assistant", "message": {"role": "assistant",
           "content": [{"type": "tool_use", "id": id, "name": name, "input": input}]}})
}

fn tool_result(id: &str, content: &str, is_error: bool) -> Value {
    json!({"type": "user", "message": {"role": "user", "content": [
        {"type": "tool_result", "tool_use_id": id, "content": content, "is_error": is_error}]}})
}

fn result(session: &str, text: &str, is_error: bool, subtype: &str) -> Value {
    json!({"type": "result", "subtype": subtype, "is_error": is_error, "result": text,
           "session_id": session, "num_turns": 1, "duration_ms": 5, "total_cost_usd": 0.001,
           "usage": {"input_tokens": 10, "output_tokens": 5}})
}

fn prompt_of(line: &str) -> Option<String> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v["type"] != "user" {
        return None;
    }
    Some(match &v["message"]["content"] {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    })
}

/// Przebieg atrapy; zwraca kod wyjścia.
pub fn run(args: &[String]) -> u8 {
    for (flag, value) in REQUIRED {
        let ok = match value {
            Some(v) => arg_value(args, flag) == Some(v),
            None => args.iter().any(|a| a == flag),
        };
        if !ok {
            eprintln!("fake-claude: brak wymaganej flagi {flag}");
            return 2;
        }
    }
    let Some(mcp_config) = arg_value(args, "--mcp-config").map(str::to_owned) else {
        eprintln!("fake-claude: brak --mcp-config");
        return 2;
    };
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let Some(prompt) = lines
        .next()
        .and_then(Result::ok)
        .and_then(|l| prompt_of(&l))
    else {
        eprintln!("fake-claude: brak wiadomości użytkownika na stdin");
        return 2;
    };
    let session = arg_value(args, "--resume").map_or_else(
        || format!("fake-sess-{}", std::process::id()),
        str::to_owned,
    );
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    emit(
        &json!({"type": "system", "subtype": "init", "session_id": session, "cwd": cwd,
                 "tools": ["Bash", "Write", "TodoWrite"], "model": "fake", "permissionMode": "default",
                 "mcp_servers": [{"name": "alfa", "status": "connected"}]}),
    );
    let code = match Scenario::parse(&prompt) {
        Scenario::Ok => {
            emit(&text("Planuję."));
            emit(&tool_use(
                "toolu_plan",
                "TodoWrite",
                json!({"todos": [
                {"content": "Przygotuj plik", "status": "completed"},
                {"content": "Podsumuj", "status": "in_progress"}]}),
            ));
            emit(&tool_result("toolu_plan", "ok", false));
            let written = std::fs::write("wynik.txt", "ok").is_ok();
            emit(&tool_use(
                "toolu_w",
                "Write",
                json!({"file_path": "wynik.txt", "content": "ok"}),
            ));
            emit(&tool_result("toolu_w", "zapisano", !written));
            emit(
                &json!({"type": "stream_event", "event": {"type": "content_block_delta", "index": 0,
                         "delta": {"type": "text_delta", "text": "Got"}}}),
            );
            emit(&text("Gotowe."));
            emit(&result(&session, "Gotowe.", false, "success"));
            0
        }
        Scenario::Permission(n) => {
            let mut link = match McpLink::connect(&mcp_config) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("fake-claude: {e}");
                    return 4;
                }
            };
            let mut decisions = Vec::new();
            for i in 0..n {
                let id = format!("toolu_{i}");
                let input = json!({"command": format!("echo {i}")});
                emit(&tool_use(&id, "Bash", input.clone()));
                match link.approve("Bash", input, &id) {
                    Ok((allow, message)) => {
                        decisions.push(allow);
                        emit(&tool_result(
                            &id,
                            if allow { "ok" } else { &message },
                            !allow,
                        ));
                    }
                    Err(e) => {
                        eprintln!("fake-claude: approve: {e}");
                        return 4;
                    }
                }
            }
            emit(&result(
                &session,
                &permission_result(&decisions),
                false,
                "success",
            ));
            0
        }
        Scenario::Slow { n, interval_ms } => {
            for _ in 0..n {
                emit(&text(&format!("t={}", now_ms())));
                std::thread::sleep(Duration::from_millis(interval_ms));
            }
            emit(&result(&session, "ok", false, "success"));
            0
        }
        Scenario::Hang => {
            let pid = spawn_grandchild();
            emit(&text(&format!("child_pid={pid}")));
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        }
        Scenario::Crash => {
            emit(&text("zaczynam"));
            return 3;
        }
        Scenario::ErrorResult => {
            emit(&result(&session, "limit tur", true, "error_max_turns"));
            1
        }
        Scenario::Garbage => {
            raw("to nie jest json");
            raw("[1,2,3]");
            emit(&json!({"type": "nieznany_typ", "x": 1}));
            emit(&json!({"type": "assistant", "message": {"content": "zamiast listy"}}));
            raw("");
            emit(&text("ok"));
            emit(&result(&session, "ok", false, "success"));
            0
        }
        Scenario::LongLine(bytes) => {
            emit(&text(&"x".repeat(bytes)));
            emit(&text("po długiej linii"));
            emit(&result(&session, "ok", false, "success"));
            0
        }
        Scenario::Env => {
            let names = env_names();
            emit(&text(&names));
            emit(&result(&session, &names, false, "success"));
            0
        }
        Scenario::Steer => {
            emit(&text("czekam na sterowanie"));
            let steer = lines
                .next()
                .and_then(Result::ok)
                .and_then(|l| prompt_of(&l))
                .unwrap_or_default();
            emit(&result(&session, "pierwsza tura", false, "success"));
            emit(&text(&format!("steer:{steer}")));
            emit(&result(
                &session,
                &format!("steer:{steer}"),
                false,
                "success",
            ));
            0
        }
    };
    // Prawdziwe CLI kończy pracę, gdy wejście stream-json dobiegnie końca.
    for _ in lines.by_ref() {}
    code
}
