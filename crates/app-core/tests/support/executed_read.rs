//! Kontrola z prawdziwym fs_read we własnym TEMP. Wybór narzędzia skryptowany, wykonanie natywne.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use accounts_hub_contract::SecretStore;
use app_core::dto::{SettingValue, WorkdirChoice};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use super::common::{self, ScriptedProvider};

pub async fn fixture() -> (common::agents::Agents, Arc<dyn SecretStore>) {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(HeadlessShell::default());
    let exec = Arc::new(platform_fake::FakeExec::new());
    let mut opts = common::options(Some(provider.clone()), shell.clone());
    let secrets = opts.secrets.clone().unwrap();
    opts.exec = Some(exec.clone());
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut a = common::agents::agents_on(core, provider, shell, dir).await;
    a.exec = exec;
    a.h.core
        .settings_set(
            "agents.interaction_mode".into(),
            SettingValue::Text("standard".into()),
        )
        .await
        .unwrap();
    let sentinel = format!("owned-read-{}", uuid::Uuid::new_v4());
    std::fs::write(a.workdir.join("fact.txt"), &sentinel).unwrap();
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "actual-read-1",
        "fs_read",
        &json!({"path":"fact.txt"}),
    ));
    a.h.provider.push(Script::text(
        FAKE_MODEL,
        &["Nie mam potwierdzenia wykonania."],
    ));
    let (_turn, _) = common::agents::run_turn(&mut a, "Przeczytaj fact.txt.").await;
    assert!(
        a.h.provider
            .requests()
            .iter()
            .any(|r| common::agents::tool_results(r).contains(&sentinel)),
        "native fs_read must return the fresh real file contents"
    );
    let runs = a.h.core.agents_runs(a.sid.clone()).await.unwrap();
    assert_eq!(
        runs.iter()
            .flat_map(|r| &r.steps)
            .filter(|s| s.tool.as_deref() == Some("fs_read"))
            .count(),
        1
    );
    assert_eq!(
        std::fs::read_to_string(a.workdir.join("fact.txt")).unwrap(),
        sentinel
    );
    // Zwykły czat w kolejnej turze: bez dostępnych narzędzi, ale z prawdziwym wcześniejszym Replay.
    a.h.core
        .sessions_choose_workdir(a.sid.clone(), WorkdirChoice::None)
        .await
        .unwrap();
    (a, secrets)
}

pub fn checked_records(request: &providers_contract::ChatRequest) -> Option<serde_json::Value> {
    let notes = record_notes(request);
    if notes.len() != 1 || notes[0].len() > 1536 {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&notes[0]).ok()?;
    let raw = serde_json::to_string(request).ok()?;
    (v["counts"]["recorded_done"] == 1
        && v["counts"]["unverified"] == 0
        && v["records"].as_array()?.iter().any(|r| {
            r["agent"] == "delta"
                && r["tool"] == "fs_read"
                && r["origin"] == "native_replay"
                && r["state"] == "recorded_done"
        })
        && !raw.contains("owned-read-")
        && request.tools.is_empty())
    .then_some(v)
}

pub fn checked_empty_records(
    request: &providers_contract::ChatRequest,
) -> Option<serde_json::Value> {
    let notes = record_notes(request);
    if notes.len() != 1 || notes[0].len() > 1536 {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&notes[0]).ok()?;
    (v["source"] == "application_execution_records"
        && v["scope"] == "selected_branch_before_current_turn"
        && v["counts"]["recorded_done"] == 0
        && v["counts"]["needs_confirmation"] == 0
        && v["counts"]["unverified"] == 0
        && v["records"].as_array()?.is_empty()
        && request.tools.is_empty())
    .then_some(v)
}

fn record_notes(request: &providers_contract::ChatRequest) -> Vec<String> {
    let marker = "[Zapis czynności aplikacji]\n";
    let user: Vec<_> = request
        .messages
        .iter()
        .filter(|m| m.role == providers_contract::Role::User)
        .filter_map(|m| m.visible_text().strip_prefix(marker).map(str::to_owned))
        .collect();
    if std::env::var_os("ALFA_CPU_DIAGNOSTIC_SECONDS").is_some()
        && std::env::var("ALFA_CPU_DIAGNOSTIC_RECORDS_SYSTEM").as_deref() == Ok("1")
    {
        if !user.is_empty() {
            return Vec::new();
        }
        return request
            .system
            .as_deref()
            .and_then(|s| s.rsplit_once(marker))
            .map(|(_, s)| vec![s.to_owned()])
            .unwrap_or_default();
    }
    user
}
