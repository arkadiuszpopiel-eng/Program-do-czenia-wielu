//! Jarvis na rzeczywistym rdzeniu z atrapą modelu: wybór każdej persony, zapis ustawień i zadania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::dto::{SessionTemplate, SettingValue, WorkdirChoice};
use app_core::{AppCore, AppPaths};
use common::{ends, harness_with, send, until};
use providers_fake::{FAKE_MODEL, Script};
use std::time::Duration;

const MARK: &str = "Tryb współpracy Jarvis";

async fn ask(
    h: &mut common::Harness,
    session: &str,
    agent: &str,
) -> providers_contract::ChatRequest {
    h.provider
        .push(Script::text(FAKE_MODEL, &["Sprawdziłam wynik."]));
    let mut message = send("Przedstaw krótko wynik zadania", None);
    message.addressed_to = Some(agent.into());
    let result = h.core.turns_send(session.into(), message).await.unwrap();
    until(&mut h.rx, ends(&result.assistant_turn_id.unwrap())).await;
    h.provider.requests().last().cloned().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn each_persona_can_use_jarvis_independently_without_changing_permissions() {
    let mut h = harness_with(Duration::from_millis(1), true).await;
    let session = h
        .core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    h.core
        .sessions_choose_workdir(session.clone(), WorkdirChoice::None)
        .await
        .unwrap();
    let before = h.core.permissions_get(Some(session.clone())).await.unwrap();
    for (agent, name) in [
        ("alfa", "Alfa"),
        ("beta", "Beta"),
        ("gama", "Gama"),
        ("delta", "Delta"),
    ] {
        let key = format!("agents.{agent}.interaction_mode");
        h.core
            .settings_set(key.clone(), SettingValue::Text("jarvis".into()))
            .await
            .unwrap();
        let request = ask(&mut h, &session, agent).await;
        let system = request.system.unwrap();
        assert!(system.contains(MARK), "{agent}");
        assert!(system.contains(name), "tożsamość {agent}");
        assert!(system.contains("rodzaju żeńskim"));
        assert!(request.tools.is_empty(), "profil nie dodaje narzędzi");
        h.core
            .settings_set(key, SettingValue::Text("standard".into()))
            .await
            .unwrap();
        assert!(
            !ask(&mut h, &session, agent)
                .await
                .system
                .unwrap()
                .contains(MARK)
        );
    }
    assert_eq!(h.core.permissions_get(Some(session)).await.unwrap(), before);
    assert_ne!(
        h.core.voice_status().await.unwrap().state,
        app_core::dto::VoiceState::Active
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn common_default_can_be_overridden_per_persona_and_survives_restart() {
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(common::ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(app_core::ports::HeadlessShell::default());
    let opts = common::options(Some(provider.clone()), shell.clone());
    // Restart używa tego samego sejfu; nowy MemorySecretStore oznaczałby utratę klucza SQLCipher.
    let secrets = opts.secrets.clone();
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let rx = core.subscribe_events();
    let mut h = common::Harness {
        core,
        provider,
        shell,
        rx,
        dir,
    };
    let session = h
        .core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    h.core
        .sessions_choose_workdir(session.clone(), WorkdirChoice::None)
        .await
        .unwrap();
    for (key, value) in [
        ("agents.interaction_mode", "jarvis"),
        ("agents.beta.interaction_mode", "standard"),
    ] {
        h.core
            .settings_set(key.into(), SettingValue::Text(value.into()))
            .await
            .unwrap();
    }
    assert!(
        ask(&mut h, &session, "alfa")
            .await
            .system
            .unwrap()
            .contains(MARK)
    );
    assert!(
        !ask(&mut h, &session, "beta")
            .await
            .system
            .unwrap()
            .contains(MARK)
    );
    let provider = h.provider.clone();
    let mut opts = common::options(Some(provider), h.shell.clone());
    opts.secrets = secrets;
    let dir = h.dir;
    drop(h.core);
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let settings = core.settings_values().await.unwrap();
    assert_eq!(
        settings["agents.interaction_mode"],
        SettingValue::Text("jarvis".into())
    );
    assert_eq!(
        settings["agents.beta.interaction_mode"],
        SettingValue::Text("standard".into())
    );
    assert!(
        core.settings_set(
            "agents.interaction_mode".into(),
            SettingValue::Text("full-access".into())
        )
        .await
        .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn jarvis_reaches_agent_run_and_keeps_selected_reply_style() {
    let mut a = common::agents::agents().await;
    a.h.core
        .settings_set(
            "agents.delta.interaction_mode".into(),
            SettingValue::Text("jarvis".into()),
        )
        .await
        .unwrap();
    a.h.core
        .settings_set(
            "chat.reply_style".into(),
            SettingValue::Text("detailed".into()),
        )
        .await
        .unwrap();
    let request = ask(&mut a.h, &a.sid, "delta").await;
    let system = request.system.unwrap();
    assert!(system.contains(MARK));
    assert!(system.contains("szczegółowo"));
    assert!(!request.tools.is_empty(), "przebieg z narzędziami");
    assert_eq!(a.h.core.agents_runs(a.sid).await.unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delegate_gets_her_own_profile_and_parent_keeps_standard() {
    let mut a = common::agents::agents().await;
    a.h.core
        .settings_set(
            "agents.delta.interaction_mode".into(),
            SettingValue::Text("jarvis".into()),
        )
        .await
        .unwrap();
    a.h.provider.push(Script::tool_call(FAKE_MODEL, "delta-task", "delegate_task", &serde_json::json!({
        "persona":"delta", "role":"operator", "goal":"Przedstaw krótki meldunek z analizy zadania"
    })));
    a.h.provider.push(Script::text(
        FAKE_MODEL,
        &["Sprawdziłam zadanie. Nie wykonywałam zmian."],
    ));
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Odebrałam meldunek Delty."]));
    let mut message = send("Przekaż Delcie analizę zadania", None);
    message.addressed_to = Some("alfa".into());
    let result = a.h.core.turns_send(a.sid, message).await.unwrap();
    until(&mut a.h.rx, ends(&result.assistant_turn_id.unwrap())).await;
    let requests = a.h.provider.requests();
    assert!(requests.len() >= 3);
    assert!(
        !requests[0]
            .system
            .as_deref()
            .unwrap_or_default()
            .contains(MARK)
    );
    assert!(
        requests[1]
            .system
            .as_deref()
            .unwrap_or_default()
            .contains(MARK),
        "delegacja zachowuje profil Delty"
    );
    assert!(
        !requests
            .last()
            .unwrap()
            .system
            .as_deref()
            .unwrap_or_default()
            .contains(MARK),
        "profil Delty nie nadpisuje Alfy"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn background_task_uses_assigned_personas_jarvis_profile() {
    use app_core::dto::{AlfaEvent, NewTaskInput, TaskStateKind};
    let mut a = common::agents::agents().await;
    a.h.core
        .settings_set(
            "agents.beta.interaction_mode".into(),
            SettingValue::Text("jarvis".into()),
        )
        .await
        .unwrap();
    a.h.provider.push(Script::text(
        FAKE_MODEL,
        &["Zebrałam informacje i sprawdziłam wynik."],
    ));
    let task =
        a.h.core
            .tasks_create(NewTaskInput {
                session_id: Some(a.sid),
                title: String::new(),
                goal: "Zbierz krótkie informacje o zadaniu".into(),
                agent: Some("beta".into()),
                after: vec![],
                parent_id: None,
            })
            .await
            .unwrap();
    until(&mut a.h.rx, |event| matches!(event, AlfaEvent::TaskUpdated { task: t } if t.id == task.id && t.state == TaskStateKind::Done)).await;
    let requests = a.h.provider.requests();
    assert!(
        requests
            .iter()
            .any(|r| r.system.as_deref().is_some_and(|s| s.contains(MARK))),
        "zadanie w tle dostaje profil Bety"
    );
}
