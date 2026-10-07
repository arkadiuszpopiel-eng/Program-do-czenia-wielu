//! Atrapa `codex app-server`: JSON-RPC po stdio bez nagłówka `jsonrpc` (założenia protokołu
//! jak w `agent-backends-impl::codex::parse` — do potwierdzenia w spike (b)).

use std::io::{BufRead, Lines, StdinLock};
use std::time::Duration;

use agent_backends_contract::contract_tests::scenario::{Scenario, permission_result};
use serde_json::{Value, json};

use crate::io::{emit, env_names, now_ms, raw, spawn_grandchild};

struct Server {
    lines: Lines<StdinLock<'static>>,
    thread: String,
    turn: u32,
    srv_id: u32,
}

fn notify(method: &str, params: Value) {
    emit(&json!({"method": method, "params": params}));
}

fn respond(id: &Value, result: Value) {
    emit(&json!({"id": id, "result": result}));
}

impl Server {
    fn next(&mut self) -> Option<Value> {
        loop {
            let line = self.lines.next()?.ok()?;
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                return Some(v);
            }
        }
    }

    fn turn_id(&self) -> String {
        format!("turn-{}", self.turn)
    }

    fn message(&self, text: &str) {
        notify(
            "item/completed",
            json!({"threadId": self.thread, "turnId": self.turn_id(),
               "item": {"type": "agentMessage", "id": format!("msg-{}", now_ms()), "text": text}}),
        );
    }

    fn complete(&self, status: &str, error: Option<&str>) {
        let mut turn = json!({"id": self.turn_id(), "status": status});
        if let Some(e) = error {
            turn["error"] = json!({"message": e});
        }
        notify(
            "turn/completed",
            json!({"threadId": self.thread, "turn": turn}),
        );
    }

    /// Wysyła żądanie zatwierdzenia i czeka na odpowiedź; `true` = zgoda.
    fn ask(&mut self, method: &str, i: u32) -> Option<bool> {
        self.srv_id += 1;
        let id = format!("srv-{}", self.srv_id);
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        emit(
            &json!({"id": id, "method": method, "params": {"threadId": self.thread, "turnId": self.turn_id(),
               "itemId": format!("item-{i}"), "command": format!("echo {i}"), "cwd": cwd, "reason": "test"}}),
        );
        loop {
            let v = self.next()?;
            if v["id"] == json!(id) {
                let d = v["result"]["decision"].as_str().unwrap_or_default();
                return Some(d == "accept" || d == "approved");
            }
        }
    }

    fn wait_request(&mut self, method: &str) -> Option<Value> {
        loop {
            let v = self.next()?;
            if v["method"] == method && v.get("id").is_some() {
                return Some(v);
            }
        }
    }

    fn start_turn(&mut self, req: &Value) -> String {
        self.turn += 1;
        respond(
            &req["id"],
            json!({"turn": {"id": self.turn_id(), "status": "inProgress", "items": []}}),
        );
        notify(
            "turn/started",
            json!({"threadId": self.thread, "turn": {"id": self.turn_id()}}),
        );
        req["params"]["input"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }

    #[allow(clippy::too_many_lines)] // jedna tabela scenariuszy
    fn scenario(&mut self, prompt: &str) -> Option<u8> {
        match Scenario::parse(prompt) {
            Scenario::Ok => {
                self.message("Planuję.");
                notify(
                    "turn/plan/updated",
                    json!({"threadId": self.thread, "turnId": self.turn_id(),
                       "plan": [{"step": "Przygotuj plik", "status": "completed"}, {"step": "Podsumuj", "status": "inProgress"}]}),
                );
                let change = json!({"type": "fileChange", "id": "fc-1", "changes": [{"path": "wynik.txt", "kind": "add"}]});
                notify("item/started", json!({"item": change}));
                let ok = std::fs::write("wynik.txt", "ok").is_ok();
                let mut done = change.clone();
                done["status"] = json!(if ok { "completed" } else { "failed" });
                notify("item/completed", json!({"item": done}));
                notify(
                    "item/started",
                    json!({"item": {"type": "commandExecution", "id": "cmd-1", "command": "ls", "cwd": "."}}),
                );
                notify(
                    "item/completed",
                    json!({"item": {"type": "commandExecution", "id": "cmd-1", "status": "completed",
                       "exitCode": 0, "aggregatedOutput": "wynik.txt"}}),
                );
                notify(
                    "item/agentMessage/delta",
                    json!({"itemId": "m", "delta": "Got"}),
                );
                self.message("Gotowe.");
                notify(
                    "thread/tokenUsage/updated",
                    json!({"tokenUsage": {"total": {"inputTokens": 10, "cachedInputTokens": 2, "outputTokens": 5}}}),
                );
                self.complete("completed", None);
            }
            Scenario::Permission(n) => {
                let mut decisions = Vec::new();
                for i in 0..n {
                    let method = if i % 2 == 0 {
                        "item/commandExecution/requestApproval"
                    } else {
                        "item/fileChange/requestApproval"
                    };
                    decisions.push(self.ask(method, i)?);
                }
                self.message(&permission_result(&decisions));
                self.complete("completed", None);
            }
            Scenario::Slow { n, interval_ms } => {
                for _ in 0..n {
                    self.message(&format!("t={}", now_ms()));
                    std::thread::sleep(Duration::from_millis(interval_ms));
                }
                self.complete("completed", None);
            }
            Scenario::Hang => {
                let pid = spawn_grandchild();
                self.message(&format!("child_pid={pid}"));
                loop {
                    std::thread::sleep(Duration::from_secs(60));
                }
            }
            Scenario::Crash => {
                self.message("zaczynam");
                return Some(3);
            }
            Scenario::ErrorResult => self.complete("failed", Some("limit tur")),
            Scenario::Garbage => {
                raw("to nie jest json");
                raw("[1,2]");
                notify("nieznana/metoda", json!({}));
                emit(&json!({"id": 999, "result": {}}));
                self.message("ok");
                self.complete("completed", None);
            }
            Scenario::LongLine(bytes) => {
                notify(
                    "item/agentMessage/delta",
                    json!({"delta": "x".repeat(bytes)}),
                );
                self.message("po długiej linii");
                self.complete("completed", None);
            }
            Scenario::Env => {
                self.message(&env_names());
                self.complete("completed", None);
            }
            Scenario::Steer => {
                self.message("czekam na sterowanie");
                let interrupt = self.wait_request("turn/interrupt")?;
                respond(&interrupt["id"], json!({}));
                self.complete("interrupted", None);
                let next = self.wait_request("turn/start")?;
                let text = self.start_turn(&next);
                self.message(&format!("steer:{text}"));
                self.complete("completed", None);
            }
        }
        None
    }
}

/// Przebieg atrapy; zwraca kod wyjścia.
pub fn run() -> u8 {
    let mut s = Server {
        lines: std::io::stdin().lock().lines(),
        thread: format!("thr-{}", std::process::id()),
        turn: 0,
        srv_id: 0,
    };
    while let Some(msg) = s.next() {
        let id = msg["id"].clone();
        match msg["method"].as_str().unwrap_or_default() {
            "initialize" => respond(&id, json!({"userAgent": "alfa-fake-agent-cli/9.8.7"})),
            "thread/start" => {
                let p = &msg["params"];
                let cwd = std::env::current_dir().ok();
                let same_cwd = p["cwd"]
                    .as_str()
                    .map(std::path::PathBuf::from)
                    .and_then(|c| c.canonicalize().ok())
                    == cwd.and_then(|c| c.canonicalize().ok());
                if p["sandbox"] != "workspace-write"
                    || p["approvalPolicy"] != "on-request"
                    || !same_cwd
                {
                    emit(
                        &json!({"id": id, "error": {"code": -32602, "message": "zły sandbox/approvalPolicy/cwd"}}),
                    );
                    continue;
                }
                respond(&id, json!({"thread": {"id": s.thread}}));
                notify("thread/started", json!({"thread": {"id": s.thread}}));
            }
            "thread/resume" => {
                s.thread = msg["params"]["threadId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                respond(&id, json!({"thread": {"id": s.thread}}));
            }
            "turn/start" => {
                let prompt = s.start_turn(&msg);
                if let Some(code) = s.scenario(&prompt) {
                    return code;
                }
            }
            "turn/interrupt" => respond(&id, json!({})),
            _ => {}
        }
    }
    0
}
