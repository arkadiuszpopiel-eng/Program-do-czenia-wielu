//! Dodatkowe scenariusze rozmowy z narzędziami: zapisany pipeline zarządzany i uruchamiany po nazwie.
//!
//! Narzędzia w zwykłej rozmowie (zgłoszenie właściciela 2026-10-20: Alfa odpowiadała „nie mam
//! dostępu do komputera”): rozmowa bez wybranego katalogu roboczego używa katalogu sesji, a Alfa
//! jako Dyrygentka (bez narzędzi) zostaje zwykłym czatem, ale wie, która agentka obsady działa
//! na komputerze i jak ją wywołać. Jawne „bez katalogu” — zwykły czat bez tej informacji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use app_core::dto::{SendOptions, SessionTemplate, SettingValue, WorkdirChoice};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, ends, options, until};
use providers_fake::{FAKE_MODEL, Script};

struct Setup {
    core: AppCore,
    provider: Arc<ScriptedProvider>,
    sid: String,
    rx: tokio::sync::broadcast::Receiver<app_core::EventBatch>,
    _dir: tempfile::TempDir,
}

/// Rdzeń z sesją, w której nikt nie wybierał katalogu roboczego.
async fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(HeadlessShell::default());
    let mut opts = options(Some(provider.clone()), shell);
    opts.approval_timeout = Some(Duration::from_secs(1));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let rx = core.subscribe_events();
    core.settings_set(
        "agents.verify_before_done".into(),
        SettingValue::Bool(false),
    )
    .await
    .unwrap();
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    Setup {
        core,
        provider,
        sid,
        rx,
        _dir: dir,
    }
}

fn message(text: &str, to: Option<&str>) -> SendOptions {
    SendOptions {
        parent_id: None,
        text: text.into(),
        addressed_to: to.map(Into::into),
        profile: None,
        attachments: Vec::new(),
    }
}

async fn ask(s: &mut Setup, text: &str, to: Option<&str>) -> String {
    let sent = s
        .core
        .turns_send(s.sid.clone(), message(text, to))
        .await
        .unwrap();
    let turn = sent.assistant_turn_id.unwrap();
    until(&mut s.rx, ends(&turn)).await;
    let snapshot = s.core.turns_list(s.sid.clone()).await.unwrap();
    snapshot
        .turns
        .iter()
        .find(|t| t.id == turn)
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn saved_pipeline_is_managed_and_runs_by_name() {
    use app_core::dto::{PipelineDef, PipelineStage};
    let mut s = setup().await;
    s.core
        .sessions_choose_workdir(s.sid.clone(), WorkdirChoice::Default)
        .await
        .unwrap();
    let lib = s.core.pipelines_list().await.unwrap();
    assert_eq!(lib.len(), 3, "trzy wzorce wbudowane");
    assert!(lib.iter().all(|d| d.builtin));
    let stage = |title: &str, role: &str, model: Option<&str>, goal: &str| PipelineStage {
        title: title.into(),
        role: role.into(),
        persona: None,
        model: model.map(Into::into),
        goal: goal.into(),
    };
    let def = PipelineDef {
        name: "Szkic do UI".into(),
        description: "Mój łańcuch".into(),
        builtin: false,
        stages: vec![
            stage("Styl", "thinker", Some("fake:astra"), "{{input}}"),
            stage("Kod", "coder", None, "Napisz kod z: {{prev}}"),
        ],
    };
    let mut bad = def.clone();
    bad.stages[0].model = Some("bez-dostawcy".into());
    assert!(
        s.core.pipelines_save(bad, None).await.is_err(),
        "zły zapis modelu"
    );
    let saved = s.core.pipelines_save(def.clone(), None).await.unwrap();
    assert_eq!(saved.len(), 4);
    assert_eq!(saved[0].name, "Szkic do UI");
    assert!(!saved[0].builtin);

    s.provider.push(Script::tool_call(
        FAKE_MODEL,
        "p1",
        "run_pipeline",
        &serde_json::json!({"pipeline": "szkic do ui", "input": "formularz logowania"}),
    ));
    s.provider.push(Script::text(FAKE_MODEL, &["STYL-OK"]));
    s.provider.push(Script::text(FAKE_MODEL, &["KOD-OK"]));
    s.provider
        .push(Script::text(FAKE_MODEL, &["Gotowe: KOD-OK."]));
    let reply = ask(&mut s, "Uruchom pipeline „Szkic do UI”.", None).await;
    assert_eq!(reply, "Gotowe: KOD-OK.");
    let requests = s.provider.requests();
    let second: String = requests[2]
        .messages
        .iter()
        .map(|m| m.visible_text())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(second.contains("Napisz kod z: STYL-OK"), "{second}");
    let tool_desc = requests[0]
        .tools
        .iter()
        .find(|t| t.name == "run_pipeline")
        .map(|t| t.description.clone())
        .unwrap_or_default();
    assert!(
        tool_desc.contains("Szkic do UI"),
        "własny pipeline w opisie narzędzia"
    );

    let after = s.core.pipelines_remove("szkic do ui".into()).await.unwrap();
    assert_eq!(after.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn explicit_no_workdir_keeps_plain_chat() {
    let mut s = setup().await;
    let none = s
        .core
        .sessions_choose_workdir(s.sid.clone(), WorkdirChoice::None)
        .await
        .unwrap();
    assert_eq!(none.path, None);
    let reply = ask(&mut s, "Co słychać?", None).await;
    assert!(reply.starts_with("Echo:"), "{reply}");
    assert!(s.core.agents_runs(s.sid.clone()).await.unwrap().is_empty());
    let system = s.provider.requests().last().cloned().unwrap().system;
    assert!(
        !system
            .unwrap_or_default()
            .contains("na komputerze użytkownika"),
        "bez katalogu nikt nie działa na komputerze"
    );
}
