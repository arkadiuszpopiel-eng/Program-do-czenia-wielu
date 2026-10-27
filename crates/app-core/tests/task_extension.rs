//! Automatyczne przedłużenie wspólnej puli wymaga jawnego ustawienia właściciela.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use app_core::dto::{SendOptions, SessionTemplate, SettingValue};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, ends, options, until};
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;
use std::{sync::Arc, time::Duration};
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn automatic_steps_complete_pipeline_beyond_original_allowance() {
    let dir = tempfile::tempdir().unwrap();
    let p = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let c = AppCore::build(
        AppPaths::under(dir.path()),
        options(Some(p.clone()), Arc::new(HeadlessShell::default())),
    )
    .await
    .unwrap();
    for (key, value) in [
        ("agents.max_steps", SettingValue::Number(5.into())),
        ("agents.auto_extend_steps", SettingValue::Bool(true)),
        ("agents.verify_before_done", SettingValue::Bool(false)),
    ] {
        c.settings_set(key.into(), value).await.unwrap();
    }
    let sid = c.sessions_create(SessionTemplate::Empty).await.unwrap().id;
    p.push(Script::tool_call(FAKE_MODEL,"p","run_pipeline",&json!({"input":"start","steps":[{"role":"thinker","persona":"gama","goal":"{{input}}"},{"role":"writer","persona":"beta","goal":"{{prev}}"}]})));
    for text in ["FIRST", "SECOND", "Wszystko gotowe"] {
        p.push(Script::text(FAKE_MODEL, &[text]));
    }
    let mut rx = c.subscribe_events();
    let turn = c
        .turns_send(
            sid.clone(),
            SendOptions {
                parent_id: None,
                text: "Wykonaj dwa etapy".into(),
                addressed_to: Some("alfa".into()),
                profile: None,
                attachments: vec![],
            },
        )
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut rx, ends(&turn)).await;
    let done = c
        .turns_list(sid.clone())
        .await
        .unwrap()
        .turns
        .into_iter()
        .find(|t| t.id == turn)
        .unwrap();
    assert!(done.error.is_none(), "{done:?}");
    assert_eq!(p.requests().len(), 4);
    let parent = c
        .agents_runs(sid)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.run.agent == "alfa" && r.run.parent_id.is_none())
        .unwrap();
    assert!(parent.run.usage.steps > 5);
    assert!(parent.run.budget.max_steps > 5);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelled_conductor_is_not_relabelled_as_paused() {
    let dir = tempfile::tempdir().unwrap();
    let p = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let c = AppCore::build(
        AppPaths::under(dir.path()),
        options(Some(p.clone()), Arc::new(HeadlessShell::default())),
    )
    .await
    .unwrap();
    let sid = c.sessions_create(SessionTemplate::Empty).await.unwrap().id;
    p.push(Script::tool_call(
        FAKE_MODEL,
        "d",
        "delegate_task",
        &json!({"persona":"delta","role":"coder","goal":"Long task"}),
    ));
    p.push(Script::stall());
    let turn = c
        .turns_send(
            sid.clone(),
            SendOptions {
                parent_id: None,
                text: "Rozpocznij zadanie".into(),
                addressed_to: Some("alfa".into()),
                profile: None,
                attachments: vec![],
            },
        )
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while p.requests().len() < 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    c.turns_stop(sid.clone()).await.unwrap();
    assert!(
        !c.agents_runs(sid.clone())
            .await
            .unwrap()
            .iter()
            .any(|r| r.run.state == app_core::dto::RunState::Paused)
    );
    assert!(c.turns_continue(sid, turn).await.is_err());
}
