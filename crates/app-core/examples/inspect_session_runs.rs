//! Read-only diagnostic for one owner-selected session. No app startup, migrations or API calls.
use app_modules::secrets::{StoreKeyVault, system_secret_store};
use core_log_contract::{Redactor, RegexRedactor};
use lib_sqlstore::rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use sessions_contract::{KeyVault, SessionId, session_key_name};
use std::collections::BTreeSet;
use std::io::Write;
fn short(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw = std::env::args().nth(1).ok_or("Pass the exact session id")?;
    if raw == "--check-undo" {
        let path = app_api::paths::AppPaths::from_env()?
            .local
            .join("undo-store/journal.ndjson");
        let text = std::fs::read_to_string(path)?;
        let lines: Vec<_> = text.lines().collect();
        let mut records = 0;
        let mut ignored_tail = false;
        for (index, line) in lines.iter().enumerate() {
            match serde_json::from_str::<undo_journal_contract::JournalRecord>(line) {
                Ok(_) => records += 1,
                Err(_) if index + 1 == lines.len() && !text.ends_with('\n') => ignored_tail = true,
                Err(_) => {
                    return Err(
                        format!("Nieczytelny rekord dziennika w linii {}", index + 1).into(),
                    );
                }
            }
        }
        writeln!(
            std::io::stdout().lock(),
            "{{\"readable_records\":{records},\"ignored_incomplete_tail\":{ignored_tail},\"mode\":\"read_only\"}}"
        )?;
        return Ok(());
    }
    let sid = SessionId::new(raw.clone());
    let path = app_api::paths::AppPaths::from_env()?
        .sessions()
        .join(format!("{raw}.db"));
    if raw.contains(['/', '\\']) || !path.is_file() {
        return Err("Invalid session file".into());
    }
    let vault = StoreKeyVault::new(system_secret_store()?);
    let key = vault
        .load(&session_key_name(&sid))?
        .ok_or("Session key unavailable")?;
    let c = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    c.execute_batch("PRAGMA cipher_log_level=NONE;")?;
    c.pragma_update(None, "key", format!("x'{}'", key.to_hex().as_str()))?;
    c.execute_batch("PRAGMA query_only=ON; PRAGMA busy_timeout=5000;")?;
    let redactor = RegexRedactor::default();
    let emit = |mut value: Value| {
        redactor.redact_value(&mut value);
        writeln!(std::io::stdout().lock(), "{value}")
    };
    let mut q = c.prepare("SELECT body FROM app_agent_runs ORDER BY seq DESC LIMIT 6")?;
    for row in q.query_map([], |r| r.get::<_, String>(0))? {
        let v: Value = serde_json::from_str(&row?)?;
        emit(
            json!({"run":v["id"],"agent":v["agent"],"state":v["state"],"summary":v["summary"],"turn":v["turn_id"]}),
        )?;
    }
    let mut q = c.prepare("SELECT body FROM app_agent_steps ORDER BY seq DESC LIMIT 300")?;
    let mut seen = BTreeSet::new();
    for row in q.query_map([], |r| r.get::<_, String>(0))? {
        let v: Value = serde_json::from_str(&row?)?;
        if !seen.insert(v["id"].to_string()) {
            continue;
        }
        let tool = v["tool"].as_str().unwrap_or("");
        if tool.contains("shell") {
            emit(
                json!({"step":v["id"],"tool":tool,"status":v["status"],"input":v["input"],"output":v["output"]}),
            )?;
        }
    }
    let mut q = c.prepare(
        "SELECT body FROM app_work_state WHERE kind='runtime' ORDER BY updated DESC LIMIT 16",
    )?;
    let mut seen = BTreeSet::new();
    for row in q.query_map([], |r| r.get::<_, String>(0))? {
        let v: Value = serde_json::from_str(&row?)?;
        if v["spec"]["agent"] == "gama" {
            emit(
                json!({"verifier_run":v["run"],"goal":v["spec"]["goal"],"finished":v["finished"]}),
            )?;
        }
        let Some(messages) = v["messages"].as_array() else {
            continue;
        };
        let blocks: Vec<&Value> = messages
            .iter()
            .filter_map(|m| m["content"].as_array())
            .flatten()
            .collect();
        for b in &blocks {
            if b["type"] != "tool_use" {
                continue;
            }
            let name = b["name"].as_str().unwrap_or("");
            if name != "shell_run" && name != "tool_call" {
                continue;
            }
            let input = b["input"].to_string();
            if !["dotnet", "build", "test", "msbuild"]
                .iter()
                .any(|s| input.to_lowercase().contains(s))
            {
                continue;
            }
            if !seen.insert(input.clone()) {
                continue;
            }
            let result = blocks
                .iter()
                .find(|r| r["type"] == "tool_result" && r["tool_use_id"] == b["id"])
                .map(|r| r["content"].to_string())
                .unwrap_or_else(|| "No recorded result".into());
            emit(
                json!({"runtime":v["run"],"agent":v["spec"]["agent"],"outcome":v["finished"],"tool":name,"input":short(&input,1200),"result":short(&result,3000)}),
            )?;
        }
    }
    Ok(())
}
