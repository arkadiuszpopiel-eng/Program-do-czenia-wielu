//! Profile narzędzi i budżet orkiestracji sprawdzane na prawdziwym rdzeniu, z atrapą modelu.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use app_core::dto::{SendOptions, SettingValue};
use common::agents::{Agents, agents};
use common::{ends, until};
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;

async fn ask(a: &mut Agents, to: &str) {
    let sent =
        a.h.core
            .turns_send(
                a.sid.clone(),
                SendOptions {
                    parent_id: None,
                    text: "Wykonaj zadanie w repozytorium".into(),
                    addressed_to: Some(to.into()),
                    profile: None,
                    attachments: Vec::new(),
                },
            )
            .await
            .unwrap();
    until(&mut a.h.rx, ends(&sent.assistant_turn_id.unwrap())).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn alfa_can_execute_a_file_step_herself_with_default_full_profile() {
    let mut a = agents().await;
    let path = a.workdir.join("alfa-full.txt");
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "self",
        "delegate_task",
        &json!({"persona":"alfa","role":"operator","goal":"Write the file"}),
    ));
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "write",
        "fs_write",
        &json!({"path":path,"content":"alfa-ok","mode":"create"}),
    ));
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Zapisalam plik."]));
    a.h.provider.push(Script::text(FAKE_MODEL, &["Gotowe."]));
    ask(&mut a, "alfa").await;
    assert_eq!(std::fs::read_to_string(path).unwrap(), "alfa-ok");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delegating_an_unassigned_operator_role_does_not_expand_beta_access() {
    let mut a = agents().await;
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "beta",
        "delegate_task",
        &json!({"persona":"beta","role":"operator","goal":"Inspect the project"}),
    ));
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Sprawdzilam."]));
    a.h.provider.push(Script::text(FAKE_MODEL, &["Gotowe."]));
    ask(&mut a, "alfa").await;
    let requests = a.h.provider.requests();
    let child = &requests[1];
    assert!(
        !child
            .tools
            .iter()
            .any(|t| t.name == "shell_run" || t.name.starts_with("gui_")),
        "{:?}",
        child.tools
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn disabled_and_custom_profiles_control_actual_model_tool_registry() {
    let mut a = agents().await;
    a.h.core
        .settings_set(
            "agents.delta.access".into(),
            SettingValue::Text("off".into()),
        )
        .await
        .unwrap();
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Bez narzedzi."]));
    ask(&mut a, "delta").await;
    assert!(a.h.provider.requests().last().unwrap().tools.is_empty());
    a.h.core
        .settings_set(
            "agents.delta.access".into(),
            SettingValue::Text("custom".into()),
        )
        .await
        .unwrap();
    a.h.core
        .settings_set(
            "agents.delta.groups".into(),
            SettingValue::Text("fs.read".into()),
        )
        .await
        .unwrap();
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Tylko pliki."]));
    ask(&mut a, "delta").await;
    let request = a.h.provider.requests().last().cloned().unwrap();
    assert!(request.tools.iter().any(|t| t.name == "fs_read"));
    assert!(
        !request
            .tools
            .iter()
            .any(|t| t.name == "fs_write" || t.name == "shell_run")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn conductor_continues_beyond_eight_rounds_within_shared_budget() {
    let mut a = agents().await;
    a.h.core
        .settings_set("agents.max_steps".into(), SettingValue::Number(500.into()))
        .await
        .unwrap();
    for i in 0..10 {
        a.h.provider.push(Script::tool_call(
            FAKE_MODEL,
            &format!("d{i}"),
            "delegate_task",
            &json!({"persona":"gama","role":"thinker","goal":format!("Analyse stage {i}")}),
        ));
        a.h.provider
            .push(Script::text(FAKE_MODEL, &["Etap wykonany."]));
    }
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Wszystkie 10 etapow wykonane."]));
    ask(&mut a, "alfa").await;
    assert_eq!(a.h.provider.requests().len(), 21);
    let runs = a.h.core.agents_runs(a.sid.clone()).await.unwrap();
    let parent = runs.iter().find(|r| r.run.parent_id.is_none()).unwrap();
    assert!(parent.run.usage.steps > 10 && parent.run.usage.steps <= 500);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn background_task_preserves_disabled_profile() {
    let mut a = agents().await;
    a.h.core
        .settings_set(
            "agents.delta.access".into(),
            SettingValue::Text("off".into()),
        )
        .await
        .unwrap();
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Bez narzedzi w tle."]));
    let task =
        a.h.core
            .tasks_create(app_core::dto::NewTaskInput {
                session_id: Some(a.sid.clone()),
                title: "Profil w tle".into(),
                goal: "Przeczytaj plik".into(),
                agent: Some("delta".into()),
                after: Vec::new(),
                parent_id: None,
            })
            .await
            .unwrap();
    until(&mut a.h.rx, |event| matches!(event, app_core::dto::AlfaEvent::TaskUpdated { task: t } if t.id == task.id && t.state == app_core::dto::TaskStateKind::Done)).await;
    assert!(a.h.provider.requests().last().unwrap().tools.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn custom_delegation_only_still_exposes_delegate_task() {
    let mut a = agents().await;
    a.h.core
        .settings_set(
            "agents.delta.access".into(),
            SettingValue::Text("custom".into()),
        )
        .await
        .unwrap();
    a.h.core
        .settings_set(
            "agents.delta.groups".into(),
            SettingValue::Text("delegate".into()),
        )
        .await
        .unwrap();
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Moge delegowac."]));
    ask(&mut a, "delta").await;
    let request = a.h.provider.requests().last().cloned().unwrap();
    assert!(request.tools.iter().any(|t| t.name == "delegate_task"));
    assert!(
        !request
            .tools
            .iter()
            .any(|t| t.name == "fs_write" || t.name == "shell_run")
    );
}
