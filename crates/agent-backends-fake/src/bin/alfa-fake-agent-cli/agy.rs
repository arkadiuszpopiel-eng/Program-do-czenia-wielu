//! Atrapa `agy -p <polecenie> --output-format stream-json --sandbox` (Antigravity CLI): zdarzenia
//! `init`, `step_update`, `result` w kształcie, który czyta parser mostu.

use std::time::Duration;

use agent_backends_contract::contract_tests::scenario::{Scenario, permission_result};
use serde_json::{Value, json};

use crate::io::{env_names, now_ms, raw, spawn_grandchild};

/// Zdarzenie w kształcie Antigravity CLI 1.2.11: `{"event": t, t: {…}}` (pole `type` z opisu
/// przenoszone do `event`, reszta do obiektu zdarzenia; `conversation_id` także obok).
fn emit(v: &Value) {
    let mut inner = v.as_object().cloned().unwrap_or_default();
    let kind = inner
        .remove("type")
        .and_then(|t| t.as_str().map(str::to_owned))
        .unwrap_or_default();
    let mut outer = serde_json::Map::new();
    outer.insert("event".into(), Value::String(kind.clone()));
    if let Some(id) = inner.get("conversation_id").cloned() {
        outer.insert("conversation_id".into(), id);
    }
    if kind == "step_update"
        && let Some(status) = inner.remove("status")
    {
        inner.insert(
            "state".into(),
            Value::String(status.as_str().unwrap_or_default().to_uppercase()),
        );
    }
    outer.insert(kind, Value::Object(inner));
    crate::io::emit(&Value::Object(outer));
}

fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn say(id: &str, text: &str) -> Value {
    json!({"type": "step_update", "step_id": id, "step_type": "agent_response", "status": "done",
           "text": text})
}

fn result(conv: &str, status: &str, response: &str) -> Value {
    json!({"type": "result", "status": status, "response": response, "conversation_id": conv,
           "usage": {"input_tokens": 10, "output_tokens": 5}, "duration_ms": 5})
}

/// Przebieg atrapy; zwraca kod wyjścia.
pub fn run(args: &[String]) -> u8 {
    if arg_value(args, "--output-format") != Some("stream-json") {
        eprintln!("fake-agy: brak --output-format stream-json");
        return 2;
    }
    if args
        .iter()
        .any(|a| a.contains("dangerously") || a == "--mode")
    {
        eprintln!("fake-agy: most nie może włączać automatycznej zgody");
        return 2;
    }
    let Some(prompt) = arg_value(args, "-p") else {
        eprintln!("fake-agy: brak polecenia -p");
        return 2;
    };
    let conv = arg_value(args, "--conversation").map_or_else(
        || format!("fake-conv-{}", std::process::id()),
        str::to_owned,
    );
    emit(&json!({"type": "init", "conversation_id": conv, "model": "fake"}));
    match Scenario::parse(prompt) {
        Scenario::Ok => {
            emit(
                &json!({"type": "step_update", "step_id": "plan", "step_type": "plan",
                         "items": [{"text": "Przygotuj plik", "done": true},
                                   {"text": "Podsumuj", "done": false}]}),
            );
            emit(
                &json!({"type": "step_update", "step_id": "t1", "step_type": "tool",
                         "status": "running", "tool": {"name": "write_file",
                         "input": {"file_path": "wynik.txt", "content": "ok"}}}),
            );
            let written = std::fs::write("wynik.txt", "ok").is_ok();
            emit(
                &json!({"type": "step_update", "step_id": "t1", "step_type": "tool",
                         "status": if written { "done" } else { "error" }, "output": "zapisano"}),
            );
            emit(
                &json!({"type": "step_update", "step_id": "r", "step_type": "agent_response",
                         "status": "running", "text_delta": "Got"}),
            );
            emit(
                &json!({"type": "step_update", "step_id": "r", "step_type": "agent_response",
                         "status": "done", "text_delta": "owe."}),
            );
            emit(&result(&conv, "SUCCESS", "Gotowe."));
            0
        }
        Scenario::Permission(n) => {
            // Bez kanału zgód: agy sam odrzuca polecenia powłoki w trybie `-p`.
            for i in 0..n {
                let id = format!("t{i}");
                emit(
                    &json!({"type": "step_update", "step_id": id, "step_type": "tool",
                             "tool": {"name": "run_command", "input": {"command": format!("echo {i}")}}}),
                );
                emit(
                    &json!({"type": "step_update", "step_id": id, "step_type": "tool",
                             "status": "denied", "message": "wymaga zgody"}),
                );
            }
            let decisions = vec![false; usize::try_from(n).unwrap_or(0)];
            emit(&result(&conv, "SUCCESS", &permission_result(&decisions)));
            0
        }
        Scenario::Slow { n, interval_ms } => {
            for i in 0..n {
                emit(&say(&format!("s{i}"), &format!("t={}", now_ms())));
                std::thread::sleep(Duration::from_millis(interval_ms));
            }
            emit(&result(&conv, "SUCCESS", "ok"));
            0
        }
        Scenario::Hang => {
            let pid = spawn_grandchild();
            emit(&say("h", &format!("child_pid={pid}")));
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        }
        Scenario::Crash => {
            emit(&say("c", "zaczynam"));
            3
        }
        Scenario::ErrorResult => {
            emit(&result(&conv, "ERROR", "limit"));
            1
        }
        Scenario::Garbage => {
            raw("to nie jest json");
            raw("[1,2,3]");
            emit(&json!({"type": "nieznany_typ", "x": 1}));
            emit(&json!({"type": "step_update", "step_type": "agent_response", "text": 5}));
            raw("");
            emit(&say("g", "ok"));
            emit(&result(&conv, "SUCCESS", "ok"));
            0
        }
        Scenario::LongLine(bytes) => {
            emit(&say("l1", &"x".repeat(bytes)));
            emit(&say("l2", "po długiej linii"));
            emit(&result(&conv, "SUCCESS", "ok"));
            0
        }
        Scenario::Env => {
            let names = env_names();
            emit(&say("e", &names));
            emit(&result(&conv, "SUCCESS", &names));
            0
        }
        Scenario::Steer => {
            emit(&say("s", "polecenie jednorazowe"));
            emit(&result(&conv, "SUCCESS", "bez sterowania"));
            0
        }
    }
}
