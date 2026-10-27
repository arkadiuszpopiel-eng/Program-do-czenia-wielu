//! Owner-selected project context follows the conversation and reaches direct agents.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use app_core::dto::{SessionTemplate, SettingValue, WorkdirChoice};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, ends, options, send, until};
use providers_fake::{FAKE_MODEL, Script};
use std::{sync::Arc, time::Duration};
async fn ask(
    c: &AppCore,
    p: &ScriptedProvider,
    sid: &str,
    agent: Option<&str>,
) -> providers_contract::ChatRequest {
    p.push(Script::text(FAKE_MODEL, &["Wynik testowy"]));
    let mut rx = c.subscribe_events();
    let mut message = send("Analiza programu i 5 zalecanych ulepszen", None);
    message.addressed_to = agent.map(str::to_owned);
    let turn = c
        .turns_send(sid.into(), message)
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut rx, ends(&turn)).await;
    p.requests().last().unwrap().clone()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changing_project_and_disabling_directory_refreshes_context() {
    let root = tempfile::tempdir().unwrap();
    let p = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let c = AppCore::build(
        AppPaths::under(root.path()),
        options(Some(p.clone()), Arc::new(HeadlessShell::default())),
    )
    .await
    .unwrap();
    let sid = c.sessions_create(SessionTemplate::Empty).await.unwrap().id;
    for name in ["project-A", "project-B"] {
        let folder = root.path().join(name);
        std::fs::create_dir_all(&folder).unwrap();
        c.projects_save(name.into(), Some(folder.to_string_lossy().into_owned()))
            .await
            .unwrap();
        c.sessions_set_project(sid.clone(), Some(name.into()))
            .await
            .unwrap();
        let request = ask(&c, &p, &sid, None).await;
        let system = request.system.unwrap();
        assert!(system.contains(&format!("\"project\":\"{name}\"")));
        assert!(system.contains(&serde_json::to_string(&folder.to_string_lossy()).unwrap()));
        if name == "project-B" {
            assert!(!system.contains("project-A"));
        }
    }
    c.sessions_set_project(sid.clone(), None).await.unwrap();
    c.sessions_choose_workdir(sid.clone(), WorkdirChoice::None)
        .await
        .unwrap();
    let request = ask(&c, &p, &sid, None).await;
    let system = request.system.unwrap();
    assert!(system.contains("\"project\":null"));
    assert!(system.contains("\"active_workdir\":null"));
    assert!(system.contains("\"target_selected\":false"));
    assert!(!system.contains("project-B"));
    assert!(request.tools.is_empty());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direct_delta_retains_project_metadata_in_runtime_request() {
    let root = tempfile::tempdir().unwrap();
    let p = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let c = AppCore::build(
        AppPaths::under(root.path()),
        options(Some(p.clone()), Arc::new(HeadlessShell::default())),
    )
    .await
    .unwrap();
    c.settings_set(
        "agents.verify_before_done".into(),
        SettingValue::Bool(false),
    )
    .await
    .unwrap();
    let sid = c.sessions_create(SessionTemplate::Empty).await.unwrap().id;
    let folder = root.path().join("selected");
    std::fs::create_dir_all(&folder).unwrap();
    c.projects_save(
        "Tłumacz AI".into(),
        Some(folder.to_string_lossy().into_owned()),
    )
    .await
    .unwrap();
    c.sessions_set_project(sid.clone(), Some("Tłumacz AI".into()))
        .await
        .unwrap();
    let request = ask(&c, &p, &sid, Some("delta")).await;
    let context = request
        .messages
        .iter()
        .map(|m| m.visible_text())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(context.contains("\"project\":\"Tłumacz AI\""));
    assert!(context.contains(&serde_json::to_string(&folder.to_string_lossy()).unwrap()));
    assert!(!request.tools.is_empty());
}
