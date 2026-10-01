//! Round-trip DTO: każdy ładunek z atrapy UI (`apps/desktop/ui/src/lib/api/fake`, wygenerowany do
//! `tests/fixtures/*.json`) deserializuje się do typów Rust i serializuje z powrotem bez strat.
//! Dodatkowo: zbiór komend w fixture'ach = zbiór komend w COMMANDS.md = `app_core::COMMANDS`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod dto_spec;

use std::collections::BTreeSet;
use std::path::PathBuf;

use app_core::dto::*;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn load(name: &str) -> Value {
    let text = std::fs::read_to_string(fixtures().join(name)).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Równość JSON z liczbami porównywanymi wartością (JS nie odróżnia `1` od `1.0`).
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

fn roundtrip<T: DeserializeOwned + Serialize>(what: &str, value: &Value) {
    let typed: T = serde_json::from_value(value.clone())
        .unwrap_or_else(|e| panic!("{what}: nie deserializuje się: {e}\n{value:#}"));
    let back = serde_json::to_value(&typed).unwrap();
    assert!(
        same(value, &back),
        "{what}: strata przy round-trip\nwe: {value:#}\nwy: {back:#}"
    );
}

fn parse_only<T: DeserializeOwned>(what: &str, value: &Value) {
    serde_json::from_value::<T>(value.clone())
        .unwrap_or_else(|e| panic!("{what}: nie deserializuje się: {e}\n{value:#}"));
}

type Check = fn(&str, &Value);

fn arg<'a>(args: &'a Value, name: &str) -> &'a Value {
    args.get(name).unwrap_or(&Value::Null)
}

/// Typy argumentów (nazwy camelCase jak w `TauriAlfaClient`) i wyniku każdej komendy.
fn spec(command: &str) -> (Vec<(&'static str, Check)>, Check) {
    let s: Check = roundtrip::<String>;
    let os: Check = roundtrip::<Option<String>>;
    let b: Check = roundtrip::<bool>;
    let unit: Check = roundtrip::<()>;
    match command {
        "app_bootstrap" => (vec![], roundtrip::<AppBootstrap>),
        "app_complete_onboarding"
        | "app_open_system_settings"
        | "app_save_layout"
        | "app_set_active_session" => (
            vec![
                ("uri", os),
                ("layout", roundtrip::<Option<LayoutPrefs>>),
                ("sessionId", os),
            ],
            unit,
        ),
        "sessions_list" => (vec![], roundtrip::<Vec<SessionSummary>>),
        "sessions_create" => (
            vec![("template", roundtrip::<SessionTemplate>)],
            roundtrip::<SessionSummary>,
        ),
        "sessions_rename"
        | "sessions_set_pinned"
        | "sessions_set_archived"
        | "sessions_mark_read"
        | "sessions_save_draft"
        | "sessions_undo_remove" => (
            vec![
                ("sessionId", os),
                ("title", os),
                ("pinned", roundtrip::<Option<bool>>),
                ("archived", roundtrip::<Option<bool>>),
                ("text", os),
                ("token", os),
            ],
            unit,
        ),
        "sessions_remove" => (vec![("sessionId", s)], roundtrip::<UndoTicket>),
        "sessions_duplicate_as_template" => (vec![("sessionId", s)], roundtrip::<SessionSummary>),
        "sessions_export" => (vec![("sessionId", s)], roundtrip::<ExportResult>),
        "sessions_search" => (vec![("query", s)], roundtrip::<Vec<SessionSearchHit>>),
        "sessions_get_draft" => (vec![("sessionId", s)], s),
        "sessions_workdir" => (vec![("sessionId", s)], roundtrip::<SessionWorkdir>),
        "sessions_choose_workdir" => (
            vec![("sessionId", s), ("choice", roundtrip::<WorkdirChoice>)],
            roundtrip::<SessionWorkdir>,
        ),
        "turns_list" => (vec![("sessionId", s)], roundtrip::<TurnsSnapshot>),
        "turns_send" => (
            vec![("sessionId", s), ("options", roundtrip::<SendOptions>)],
            roundtrip::<SendResult>,
        ),
        "turns_regenerate" => (vec![("sessionId", s), ("turnId", s), ("profile", os)], s),
        "turns_edit_and_resend" => (
            vec![("sessionId", s), ("turnId", s), ("text", s)],
            roundtrip::<SendResult>,
        ),
        "turns_continue" => (vec![("sessionId", s), ("turnId", s)], s),
        "turns_stop" => (vec![("sessionId", s)], unit),
        "turns_rate" => (
            vec![("turnId", s), ("rating", roundtrip::<Option<Rating>>)],
            unit,
        ),
        "turns_set_hidden" => (vec![("turnId", s), ("hidden", b)], unit),
        "turns_remember" => (
            vec![("turnId", s), ("scope", roundtrip::<RememberScope>)],
            unit,
        ),
        "turns_read_aloud" => (vec![("turnId", s)], unit),
        "turns_save_code" => (vec![("turnId", s), ("blockIndex", roundtrip::<u64>)], unit),
        "turns_run_code" => (
            vec![("turnId", s), ("blockIndex", roundtrip::<u64>)],
            roundtrip::<BrokerIntentResult>,
        ),
        "turns_undo_step" => (vec![("undoToken", s)], unit),
        "agents_list" => (vec![("sessionId", s)], roundtrip::<Vec<AgentState>>),
        "agents_set_roles" => (
            vec![
                ("sessionId", s),
                ("agent", s),
                ("roleIds", roundtrip::<Vec<String>>),
            ],
            unit,
        ),
        "agents_apply_cast" => (
            vec![("sessionId", s), ("template", roundtrip::<CastTemplateId>)],
            unit,
        ),
        "agents_runs" => (vec![("sessionId", s)], roundtrip::<Vec<AgentRunDetail>>),
        "agents_steer" => (vec![("sessionId", s), ("text", s)], unit),
        "agents_open_terminal" => (vec![("stepId", s)], unit),
        "costs_summary" => (vec![("sessionId", os)], roundtrip::<CostSummary>),
        "costs_set_monthly_limit" => (vec![("enabled", b), ("monthly", roundtrip::<Money>)], unit),
        "settings_schema" => (vec![], roundtrip::<Vec<SettingsPageDef>>),
        "settings_values" => (
            vec![],
            roundtrip::<std::collections::BTreeMap<String, SettingValue>>,
        ),
        "settings_set" => (vec![("key", s), ("value", roundtrip::<SettingValue>)], unit),
        "settings_reset" => (vec![("key", s)], roundtrip::<SettingValue>),
        "settings_set_shortcut" => (vec![("actionId", s), ("chord", os)], unit),
        "timeline_list" => (
            vec![("sessionId", s), ("filter", roundtrip::<TimelineFilter>)],
            roundtrip::<Vec<TimelineEvent>>,
        ),
        "files_list" => (vec![("sessionId", s)], roundtrip::<Vec<ArtifactInfo>>),
        "files_preview" => (vec![("artifactId", s)], roundtrip::<ArtifactPreview>),
        "files_act" => (
            vec![("artifactId", s), ("action", roundtrip::<ArtifactAction>)],
            unit,
        ),
        "accounts_catalog" => (vec![], roundtrip::<Vec<ProviderInfo>>),
        "accounts_list" => (vec![], roundtrip::<Vec<Account>>),
        "accounts_add" => (
            vec![("input", parse_only::<AddAccountInput>)],
            roundtrip::<Account>,
        ),
        "accounts_test" => (vec![("accountId", s)], roundtrip::<TestReport>),
        "accounts_assign" => (
            vec![
                ("accountId", s),
                ("assignment", roundtrip::<AccountAssignment>),
            ],
            unit,
        ),
        "accounts_set_limit" => (
            vec![
                ("accountId", s),
                ("enabled", b),
                ("monthly", roundtrip::<Money>),
            ],
            unit,
        ),
        "accounts_remove" => (vec![("accountId", s)], unit),
        "transfer_export" => (
            vec![("request", parse_only::<ExportRequest>)],
            roundtrip::<ExportResult>,
        ),
        "transfer_export_secrets" => (
            vec![("password", parse_only::<SecretInput>)],
            roundtrip::<ExportResult>,
        ),
        "transfer_inspect" => (
            vec![("password", os), ("path", os)],
            roundtrip::<InspectResult>,
        ),
        "transfer_import" => (
            vec![("request", parse_only::<ImportRequest>)],
            roundtrip::<ImportResult>,
        ),
        "transfer_rollback" => (vec![("snapshotId", s)], unit),
        "permissions_get" => (vec![("sessionId", os)], roundtrip::<PermissionsState>),
        "permissions_request_level" => (
            vec![("level", roundtrip::<AutonomyLevel>), ("sessionId", os)],
            roundtrip::<BrokerIntentResult>,
        ),
        "permissions_open_approval" => (vec![("approvalId", s)], roundtrip::<BrokerIntentResult>),
        "models_local_list" => (vec![], roundtrip::<Vec<LocalModelInfo>>),
        "models_local_download" | "models_local_cancel" => (vec![("modelId", os)], unit),
        "device_profile" | "device_measure" => (vec![], roundtrip::<DeviceProfile>),
        "voice_devices" => (vec![], roundtrip::<Vec<AudioDevice>>),
        "voice_start_mic_test"
        | "voice_stop_mic_test"
        | "voice_set_mic_enabled"
        | "voice_set_muted"
        | "voice_stop_speech" => (
            vec![
                ("deviceId", os),
                ("enabled", roundtrip::<Option<bool>>),
                ("muted", roundtrip::<Option<bool>>),
            ],
            unit,
        ),
        "voice_status" => (vec![], roundtrip::<VoiceStatus>),
        "voice_ptt" => (vec![("pressed", b)], unit),
        "voice_preview" => (vec![("agent", s)], unit),
        "system_status" => (vec![], roundtrip::<SystemStatus>),
        "system_retry_queue" | "quick_hide" => (vec![], unit),
        "quick_ask" => (vec![("text", s)], roundtrip::<QuickAskResult>),
        "quick_expand_to_main" => (vec![("sessionId", s)], unit),
        other => dto_spec::spec(other)
            .unwrap_or_else(|| panic!("komenda bez specyfikacji w teście: {other}")),
    }
}

fn fixture_entries() -> Vec<Value> {
    let mut all = Vec::new();
    for entry in std::fs::read_dir(fixtures()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.extension().is_some_and(|e| e == "json")
            && name != "events.json"
            && name != "extra.json"
        {
            all.extend(load(&name).as_array().unwrap().iter().cloned());
        }
    }
    all
}

#[test]
fn every_command_payload_roundtrips() {
    let entries = fixture_entries();
    assert!(
        entries.len() >= 150,
        "za mało przykładów: {}",
        entries.len()
    );
    for entry in &entries {
        let command = entry["command"].as_str().unwrap();
        let (args, result) = spec(command);
        let given = entry["args"].as_object().unwrap();
        let known: BTreeSet<&str> = args.iter().map(|(n, _)| *n).collect();
        for name in given.keys() {
            assert!(
                known.contains(name.as_str()),
                "{command}: nieznany argument `{name}`"
            );
        }
        for (name, check) in args {
            check(&format!("{command}.{name}"), arg(&entry["args"], name));
        }
        result(&format!("{command} → wynik"), &entry["result"]);
    }
}

#[test]
fn every_event_sample_roundtrips() {
    let events = load("events.json");
    let extra = load("extra.json");
    let all: Vec<&Value> = events
        .as_array()
        .unwrap()
        .iter()
        .chain(extra["events"].as_array().unwrap())
        .collect();
    let mut types = BTreeSet::new();
    for event in all {
        roundtrip::<AlfaEvent>("zdarzenie", event);
        types.insert(event["type"].as_str().unwrap().to_owned());
    }
    // Wszystkie 29 typów z COMMANDS.md (tabela „Zdarzenia").
    assert_eq!(types.len(), 29, "typy zdarzeń w fixture'ach: {types:?}");
}

#[test]
fn extra_result_variants_roundtrip() {
    let extra = load("extra.json");
    let results = &extra["results"];
    for v in results["ExportResult"].as_array().unwrap() {
        roundtrip::<ExportResult>("ExportResult", v);
    }
    for v in results["InspectResult"].as_array().unwrap() {
        roundtrip::<InspectResult>("InspectResult", v);
    }
    for v in results["ArtifactPreview"].as_array().unwrap() {
        roundtrip::<ArtifactPreview>("ArtifactPreview", v);
    }
    for v in results["SystemStatus"].as_array().unwrap() {
        roundtrip::<SystemStatus>("SystemStatus", v);
    }
    for v in results["PermissionsState"].as_array().unwrap() {
        roundtrip::<PermissionsState>("PermissionsState", v);
    }
    for v in results["TestReport"].as_array().unwrap() {
        roundtrip::<TestReport>("TestReport", v);
    }
    for v in results["DeviceProfile"].as_array().unwrap() {
        roundtrip::<DeviceProfile>("DeviceProfile", v);
    }
}

/// Nazwy komend z tabeli „Komendy" w COMMANDS.md.
fn commands_md() -> BTreeSet<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/ui/src/lib/api/COMMANDS.md");
    let text = std::fs::read_to_string(path).unwrap();
    let section = text.split("## Komendy").nth(1).unwrap();
    let section = section.split("## Zdarzenia").next().unwrap();
    let mut out = BTreeSet::new();
    for line in section.lines().filter(|l| l.starts_with("| `")) {
        let first = line.split('|').nth(1).unwrap();
        for part in first.split('`').skip(1).step_by(2) {
            out.insert(part.to_owned());
        }
    }
    out
}

#[test]
fn fixtures_cover_commands_md_and_app_core_implements_them() {
    let md = commands_md();
    assert!(md.len() >= 60, "COMMANDS.md: {} komend", md.len());
    let covered: BTreeSet<String> = fixture_entries()
        .iter()
        .map(|e| e["command"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(covered, md, "fixture'y ≠ COMMANDS.md");
    let implemented: BTreeSet<String> =
        app_core::COMMANDS.iter().map(|c| (*c).to_owned()).collect();
    assert_eq!(implemented, md, "app_core::COMMANDS ≠ COMMANDS.md");
}
