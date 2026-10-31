//! Przegląd ogólny narzędzi agentek (docs/plan/45 §6.4): KAŻDE narzędzie rdzenia — także dodane
//! później — ma poprawny manifest (nazwa dla modelu, opis, grupy, zamknięty schemat argumentów)
//! i odrzuca błędne argumenty (`InvalidArgs`), zanim cokolwiek zrobi. Rdzeń na atrapach pulpitu,
//! schowka, wykonania poleceń i systemu plików — żadne wywołanie nie sięga do pulpitu właściciela.
//! Działanie z poprawnymi argumentami sprawdzają testy modułów (`crates/tools-*-impl/tests`).
//! katalog-przeglad: narzedzie

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use platform_fake::{FakeClipboard, FakeDesktop, FakeExec};
use risk_classifier_contract::Reversibility;
use safety_broker_contract::Holder;
use serde_json::{Value, json};
use tools_common_contract::{Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, ToolStatus};

/// Rdzeń z atrapami wszystkich portów, przez które narzędzia działają na maszynie.
async fn core(dir: &std::path::Path) -> AppCore {
    let mut opts = common::options(None, Arc::new(HeadlessShell::default()));
    opts.exec = Some(Arc::new(FakeExec::new()));
    opts.clipboard = Some(Arc::new(FakeClipboard::new()));
    opts.gui = Some(app_gui::GuiPorts::from_one(
        Arc::new(FakeDesktop::new()),
        true,
    ));
    opts.approval_timeout = Some(Duration::from_millis(200));
    AppCore::build(AppPaths::under(dir), opts).await.unwrap()
}

/// Problemy manifestu narzędzia (pusta lista — manifest poprawny).
fn manifest_problems(tool: &dyn Tool) -> Vec<String> {
    let m = tool.manifest();
    let mut out = Vec::new();
    let valid_name = !m.name.is_empty()
        && m.name.len() <= 64
        && m.name.starts_with(|c: char| c.is_ascii_lowercase())
        && m.name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valid_name {
        out.push("nazwa nie pasuje do [a-z][a-z0-9_]{0,63}".into());
    }
    if m.title.trim().is_empty() || m.description.trim().chars().count() < 20 {
        out.push("brak tytułu albo opis krótszy niż 20 znaków".into());
    }
    if m.groups.is_empty() {
        out.push("brak grup (filtr ról nie przydzieli narzędzia)".into());
    }
    let schema = &m.input_schema;
    if schema["type"] != "object" {
        out.push(format!(
            "schemat argumentów nie jest obiektem: {}",
            schema["type"]
        ));
    }
    if schema["additionalProperties"] != Value::Bool(false) {
        out.push("schemat argumentów nie jest zamknięty (additionalProperties ≠ false)".into());
    }
    let props: BTreeSet<&str> = schema["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default();
    for req in required(tool) {
        if !props.contains(req.as_str()) {
            out.push(format!(
                "wymagany argument „{req}” nie ma opisu w properties"
            ));
        }
    }
    out
}

fn required(tool: &dyn Tool) -> Vec<String> {
    tool.manifest().input_schema["required"]
        .as_array()
        .map(|r| {
            r.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Wywołanie z argumentami, które muszą zostać odrzucone; `None` — odrzucone poprawnie.
async fn rejects(tool: &dyn Tool, args: Value, ctx: &ToolCtx, what: &str) -> Option<String> {
    let call = tool.call(args, ctx);
    let Ok(outcome) = tokio::time::timeout(Duration::from_secs(10), call).await else {
        return Some(format!("{what}: brak odpowiedzi w 10 s"));
    };
    match outcome.status {
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs,
        } if !outcome.text.trim().is_empty() => None,
        other => Some(format!(
            "{what}: {other:?} zamiast InvalidArgs — {}",
            outcome.text
        )),
    }
}

/// Wszystkie problemy jednego narzędzia: manifest i odrzucanie błędnych argumentów.
async fn tool_problems(tool: &dyn Tool, ctx: &ToolCtx) -> Vec<String> {
    let mut problems = manifest_problems(tool);
    // Nieznany argument — schemat zamknięty, więc narzędzie nie może go przemilczeć.
    let unknown = json!({ "__argument_spoza_schematu": true });
    problems.extend(rejects(tool, unknown, ctx, "nieznany argument").await);
    if !required(tool).is_empty() {
        problems.extend(rejects(tool, json!({}), ctx, "brak wymaganych").await);
    }
    problems.extend(rejects(tool, json!("tekst"), ctx, "argumenty nie-obiekt").await);
    problems
}

fn ctx(workdir: &std::path::Path) -> ToolCtx {
    ToolCtx::new(Holder::agent("s1", "delta")).with_workdir(&workdir.to_string_lossy())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_agent_tool_has_a_sound_manifest_and_rejects_bad_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let core = core(dir.path()).await;
    let tools = core.agent_tools();
    assert!(tools.len() >= 60, "za mało narzędzi: {}", tools.len());
    let workdir = dir.path().join("praca");
    std::fs::create_dir_all(&workdir).unwrap();
    let ctx = ctx(&workdir);

    let mut names = BTreeSet::new();
    let mut problems = Vec::new();
    for tool in &tools {
        let name = tool.manifest().name.clone();
        if !names.insert(name.clone()) {
            problems.push(format!("{name}: nazwa powtórzona"));
        }
        for p in tool_problems(tool.as_ref(), &ctx).await {
            problems.push(format!("{name}: {p}"));
        }
    }
    assert!(
        problems.is_empty(),
        "{} narzędzi, problemy ({}):\n{}",
        tools.len(),
        problems.len(),
        problems.join("\n")
    );
}

/// Narzędzie, które przyjmuje wszystko (kontrola przeglądu).
struct Sloppy(ToolManifest);

#[async_trait::async_trait]
impl Tool for Sloppy {
    fn manifest(&self) -> &ToolManifest {
        &self.0
    }

    async fn call(&self, _args: Value, _ctx: &ToolCtx) -> ToolOutcome {
        ToolOutcome::ok("wykonano (atrapa)", json!({}))
    }
}

/// Kontrola przeglądu: narzędzie z otwartym schematem, które przyjmuje wszystko, musi zostać
/// zgłoszone w każdym punkcie — inaczej zielony przegląd niczego by nie dowodził.
#[tokio::test]
async fn the_sweep_flags_a_tool_that_accepts_anything() {
    let sloppy = Sloppy(ToolManifest {
        name: "Zła nazwa".into(),
        id: "test.sloppy".into(),
        title: "Atrapa".into(),
        description: "krótki".into(),
        input_schema: json!({
            "type": "object",
            "properties": { "x": { "type": "string" } },
            "required": ["y"]
        }),
        output_schema: json!({}),
        reversible: Reversibility::Yes,
        capabilities: vec![],
        groups: vec![],
        mutating: false,
        untrusted_output: None,
    });
    let dir = tempfile::tempdir().unwrap();
    let problems = tool_problems(&sloppy, &ctx(dir.path())).await;
    for expected in [
        "nazwa",
        "opis krótszy",
        "brak grup",
        "nie jest zamknięty",
        "„y”",
        "nieznany argument",
        "brak wymaganych",
        "nie-obiekt",
    ] {
        assert!(
            problems.iter().any(|p| p.contains(expected)),
            "przegląd nie zgłosił „{expected}”: {problems:?}"
        );
    }
}
