//! Jarvis dla własnej persony: publiczny Kreator, obsada, rozmowa i ponowne uruchomienie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::dto::{AgentDraft, SettingValue, VoiceState};
use app_core::{AppCore, AppPaths};
use common::{Harness, ends, send, until};
use providers_contract::ChatRequest;
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

const MARK: &str = "Tryb współpracy Jarvis";
const IDENTITY: &str = "Tożsamość tej agentki: imię „Luna”, forma „Jestem Luną”.";

fn draft() -> AgentDraft {
    serde_json::from_value(json!({
        "name": "Luna", "glyph": "L", "color": "color.agent.custom-2",
        "character": "Spokojna, konkretna.",
        "voice": {"base": "pl-f2", "pitch": 1.08, "rate": 1.0, "perceived_age": 22,
                  "timbre": "ciepła",
                  "design_prompt": "młoda dorosła kobieta, ok. 22 lat, ciepła, czysta polszczyzna"},
        "role": {"id": "czytelniczka", "name": "Czytelniczka", "description": "Czyta pliki.",
                 "prompt": "Czytam pliki i mówię, co sprawdziłam.", "model_policy": "conversation",
                 "tools": ["fs"], "read_only": true, "untrusted_isolated": false, "author": false},
        "limits": {"autonomy": "L1", "memory_scope": "agent", "retain_days": 30}
    }))
    .unwrap()
}

async fn create_persona(core: &AppCore, session: &str) {
    let preview = core.builder_preview(draft()).await.unwrap();
    assert_eq!(preview.persona_id, "luna");
    assert!(preview.forms.iter().any(|form| form == "Luną"));
    let dry = core.builder_dry_run(draft()).await.unwrap();
    assert!(dry.passed, "{dry:?}");
    assert_eq!(dry.hash, preview.hash);
    let saved = core.builder_save(draft(), dry.hash).await.unwrap();
    assert_eq!(saved.persona, "luna");
    assert_eq!(saved.role, "czytelniczka");
    let library = core.builder_library().await.unwrap();
    let entry = library.iter().find(|a| a.persona == saved.persona).unwrap();
    assert_eq!(entry.role, "Czytelniczka", "nazwa do wyświetlenia");
    let wire = serde_json::to_value(entry).unwrap();
    assert_eq!(wire["role_id"], saved.role, "ID do przypisania, nie nazwa");
    core.agents_set_roles(
        session.into(),
        saved.persona,
        vec![wire["role_id"].as_str().unwrap().into()],
    )
    .await
    .unwrap();
}

async fn ask(h: &mut Harness, session: &str) -> ChatRequest {
    h.provider
        .push(Script::text(FAKE_MODEL, &["Sprawdziłam opis zadania."]));
    let mut message = send("Przedstaw krótki meldunek", None);
    message.addressed_to = Some("luna".into());
    let result = h.core.turns_send(session.into(), message).await.unwrap();
    until(&mut h.rx, ends(&result.assistant_turn_id.unwrap())).await;
    h.provider.requests().last().cloned().unwrap()
}

fn check_profile(request: &ChatRequest, jarvis: bool) {
    let system = request.system.as_deref().unwrap();
    assert_eq!(system.contains(MARK), jarvis);
    assert_eq!(system.contains(IDENTITY), jarvis);
    assert!(
        system.contains("Luna"),
        "własna tożsamość także w Standardzie"
    );
    assert!(system.contains("Czytam pliki i mówię, co sprawdziłam."));
    assert!(system.contains("wyłącznie odczyt"));
    assert!(!request.tools.is_empty(), "własna rola zachowuje narzędzia");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn builder_persona_keeps_roles_and_jarvis_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(common::ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(app_core::ports::HeadlessShell::default());
    let exec = Arc::new(platform_fake::FakeExec::new());
    let mut opts = common::options(Some(provider.clone()), shell.clone());
    opts.exec = Some(exec.clone());
    // Ten sam sejf utrzymuje klucz zaszyfrowanej bazy po ponownym uruchomieniu.
    let secrets = opts.secrets.clone();
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut a = common::agents::agents_on(core, provider, shell, dir).await;
    create_persona(&a.h.core, &a.sid).await;
    let before = a.h.core.permissions_get(Some(a.sid.clone())).await.unwrap();
    let standard = ask(&mut a.h, &a.sid).await;
    check_profile(&standard, false);
    a.h.core
        .settings_set(
            "agents.interaction_mode".into(),
            SettingValue::Text("jarvis".into()),
        )
        .await
        .unwrap();
    let jarvis = ask(&mut a.h, &a.sid).await;
    check_profile(&jarvis, true);
    assert_eq!(
        serde_json::to_value(&jarvis.tools).unwrap(),
        serde_json::to_value(&standard.tools).unwrap(),
        "Jarvis nie rozszerza narzędzi własnej roli"
    );

    let mut opts = common::options(Some(a.h.provider.clone()), a.h.shell.clone());
    opts.secrets = secrets;
    opts.exec = Some(exec);
    drop(a.h.core);
    let core = AppCore::build(AppPaths::under(a.h.dir.path()), opts)
        .await
        .unwrap();
    a.h.core = core;
    a.h.rx = a.h.core.subscribe_events();
    let agents = a.h.core.agents_list(a.sid.clone()).await.unwrap();
    assert!(agents.iter().any(|agent| agent.id == "luna"));
    let luna = agents.iter().find(|agent| agent.id == "luna").unwrap();
    assert!(luna.role_ids.iter().any(|role| role == "czytelniczka"));
    let resumed = ask(&mut a.h, &a.sid).await;
    check_profile(&resumed, true);
    assert_eq!(
        serde_json::to_value(&resumed.tools).unwrap(),
        serde_json::to_value(&standard.tools).unwrap()
    );
    a.h.core
        .settings_set(
            "agents.interaction_mode".into(),
            SettingValue::Text("standard".into()),
        )
        .await
        .unwrap();
    check_profile(&ask(&mut a.h, &a.sid).await, false);
    assert_eq!(a.h.core.permissions_get(Some(a.sid)).await.unwrap(), before);
    assert_ne!(
        a.h.core.voice_status().await.unwrap().state,
        VoiceState::Active
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delegation_to_builder_persona_uses_her_identity_and_read_only_role() {
    let mut a = common::agents::agents().await;
    create_persona(&a.h.core, &a.sid).await;
    for (key, value) in [
        ("agents.interaction_mode", "jarvis"),
        ("agents.alfa.interaction_mode", "standard"),
    ] {
        a.h.core
            .settings_set(key.into(), SettingValue::Text(value.into()))
            .await
            .unwrap();
    }
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "luna-task",
        "delegate_task",
        &json!({"persona": "luna", "role": "czytelniczka",
                "goal": "Sprawdź opis zadania i przedstaw krótki meldunek"}),
    ));
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Sprawdziłam opis zadania."]));
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Odebrałam meldunek Luny."]));
    let mut message = send("Przekaż Lunie analizę opisu zadania", None);
    message.addressed_to = Some("alfa".into());
    let result = a.h.core.turns_send(a.sid, message).await.unwrap();
    until(&mut a.h.rx, ends(&result.assistant_turn_id.unwrap())).await;
    let requests = a.h.provider.requests();
    assert_eq!(
        requests.len(),
        3,
        "rodzic, własna podagentka, meldunek rodzica"
    );
    check_profile(&requests[1], true);
    assert!(
        !requests[1]
            .tools
            .iter()
            .any(|tool| tool.name == "shell_run")
    );
    assert!(!requests[1].tools.iter().any(|tool| tool.name == "fs_write"));
    for parent in [&requests[0], &requests[2]] {
        let system = parent.system.as_deref().unwrap();
        assert!(!system.contains(MARK));
        assert!(!system.contains(IDENTITY));
    }
}
