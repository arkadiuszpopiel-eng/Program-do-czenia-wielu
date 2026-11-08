//! Faktyczny restart rdzenia, szyfrowany zapis i zgoda na kontynuację bez ponawiania zapisu.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use app_core::dto::{SendOptions, SessionTemplate, SettingValue, TurnErrorCode, WorkdirChoice};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, ends, options, until};
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restart_and_continue_preserves_file_progress() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let opts = options(Some(provider.clone()), Arc::new(HeadlessShell::default()));
    let secrets = opts.secrets.clone();
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    core.settings_set("agents.max_steps".into(), SettingValue::Number(5.into()))
        .await
        .unwrap();
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
    let folder = core
        .sessions_choose_workdir(sid.clone(), WorkdirChoice::Default)
        .await
        .unwrap()
        .path
        .unwrap();
    let path = std::path::Path::new(&folder).join("once.txt");
    provider.push(Script::tool_call(
        FAKE_MODEL,
        "w",
        "fs_write",
        &json!({"path":path,"content":"once","mode":"create"}),
    ));
    for id in ["r1", "r2"] {
        provider.push(Script::tool_call(
            FAKE_MODEL,
            id,
            "fs_read",
            &json!({"path":path}),
        ));
    }
    let mut rx = core.subscribe_events();
    let old = core
        .turns_send(
            sid.clone(),
            SendOptions {
                parent_id: None,
                text: "Write and verify a file".into(),
                addressed_to: Some("delta".into()),
                profile: None,
                attachments: vec![],
            },
        )
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut rx, ends(&old)).await;
    let turn = core
        .turns_list(sid.clone())
        .await
        .unwrap()
        .turns
        .into_iter()
        .find(|t| t.id == old)
        .unwrap();
    assert_eq!(turn.error.unwrap().code, TurnErrorCode::TaskPaused);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "once");
    drop(core);
    let mut opts = options(Some(provider.clone()), Arc::new(HeadlessShell::default()));
    opts.secrets = secrets;
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut rx = core.subscribe_events();
    provider.push(Script::text(FAKE_MODEL, &["Dokończone z zapisu."]));
    let next = core.turns_continue(sid.clone(), old.clone()).await.unwrap();
    until(&mut rx, ends(&next)).await;
    let turns = core.turns_list(sid).await.unwrap().turns;
    let resumed = turns.iter().find(|t| t.id == next).unwrap();
    assert!(resumed.error.is_none(), "{resumed:?}");
    assert!(
        provider
            .requests()
            .last()
            .unwrap()
            .messages
            .iter()
            .any(|m| m
                .content
                .iter()
                .any(|b| matches!(b, providers_contract::ContentBlock::ToolResult(_))))
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "once");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn conductor_continues_next_stage_after_restarting_at_limit() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let opts = options(Some(provider.clone()), Arc::new(HeadlessShell::default()));
    let secrets = opts.secrets.clone();
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    core.settings_set("agents.max_steps".into(), SettingValue::Number(5.into()))
        .await
        .unwrap();
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
    provider.push(Script::tool_call(
        FAKE_MODEL,
        "d1",
        "delegate_task",
        &json!({"persona":"delta","role":"coder","goal":"First stage"}),
    ));
    provider.push(Script::tool_call(
        FAKE_MODEL,
        "read",
        "fs_list",
        &json!({"path":"."}),
    ));
    provider.push(Script::text(FAKE_MODEL, &["Pierwszy etap gotowy."]));
    let mut rx = core.subscribe_events();
    let old = core
        .turns_send(
            sid.clone(),
            SendOptions {
                parent_id: None,
                text: "Execute two stages".into(),
                addressed_to: Some("alfa".into()),
                profile: None,
                attachments: vec![],
            },
        )
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut rx, ends(&old)).await;
    let turn = core
        .turns_list(sid.clone())
        .await
        .unwrap()
        .turns
        .into_iter()
        .find(|t| t.id == old)
        .unwrap();
    let spent_before = turn.usage.as_ref().unwrap().input_tokens;
    assert_eq!(turn.error.unwrap().code, TurnErrorCode::TaskPaused);
    drop(core);
    let mut opts = options(Some(provider.clone()), Arc::new(HeadlessShell::default()));
    opts.secrets = secrets;
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut rx = core.subscribe_events();
    provider.push(Script::tool_call(
        FAKE_MODEL,
        "d2",
        "delegate_task",
        &json!({"persona":"gama","role":"thinker","goal":"Second stage"}),
    ));
    provider.push(Script::text(FAKE_MODEL, &["Drugi etap gotowy."]));
    provider.push(Script::text(FAKE_MODEL, &["Oba etapy gotowe."]));
    let next = core.turns_continue(sid.clone(), old.clone()).await.unwrap();
    until(&mut rx, ends(&next)).await;
    let requests = provider.requests();
    assert!(requests[3].messages.iter().any(|m| m.content.iter().any(
        |b| matches!(b,providers_contract::ContentBlock::ToolResult(r) if r.tool_use_id=="d1")
    )));
    let resumed = core
        .turns_list(sid.clone())
        .await
        .unwrap()
        .turns
        .into_iter()
        .find(|t| t.id == next)
        .unwrap();
    assert!(resumed.error.is_none());
    assert!(
        core.turns_continue(sid.clone(), old).await.is_err(),
        "stary zapis nie może ponownie wykonywać ukończonej pracy"
    );
    let total = core
        .agents_runs(sid)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.run.agent == "alfa" && r.run.parent_id.is_none())
        .unwrap()
        .run
        .usage
        .input_tokens;
    assert_eq!(
        spent_before + resumed.usage.as_ref().unwrap().input_tokens,
        total,
        "kontynuacja nie może naliczać ponownie starego zużycia"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pipeline_restart_keeps_cursor_and_previous_output() {
    let dir = tempfile::tempdir().unwrap();
    let p = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let o = options(Some(p.clone()), Arc::new(HeadlessShell::default()));
    let secrets = o.secrets.clone();
    let c = AppCore::build(AppPaths::under(dir.path()), o)
        .await
        .unwrap();
    c.settings_set("agents.max_steps".into(), SettingValue::Number(5.into()))
        .await
        .unwrap();
    c.settings_set(
        "agents.verify_before_done".into(),
        SettingValue::Bool(false),
    )
    .await
    .unwrap();
    let sid = c.sessions_create(SessionTemplate::Empty).await.unwrap().id;
    p.push(Script::tool_call(FAKE_MODEL,"p","run_pipeline",&json!({"input":"first-input","steps":[{"role":"thinker","persona":"gama","goal":"{{input}}"},{"role":"writer","persona":"beta","goal":"Continue from {{prev}}"}]})));
    p.push(Script::text(FAKE_MODEL, &["FIRST-OUTPUT"]));
    let mut rx = c.subscribe_events();
    let old = c
        .turns_send(
            sid.clone(),
            SendOptions {
                parent_id: None,
                text: "Uruchom pipeline".into(),
                addressed_to: Some("alfa".into()),
                profile: None,
                attachments: vec![],
            },
        )
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut rx, ends(&old)).await;
    assert_eq!(
        c.turns_list(sid.clone())
            .await
            .unwrap()
            .turns
            .into_iter()
            .find(|t| t.id == old)
            .unwrap()
            .error
            .unwrap()
            .code,
        TurnErrorCode::TaskPaused
    );
    drop(c);
    let mut o = options(Some(p.clone()), Arc::new(HeadlessShell::default()));
    o.secrets = secrets;
    let c = AppCore::build(AppPaths::under(dir.path()), o)
        .await
        .unwrap();
    let mut rx = c.subscribe_events();
    p.push(Script::text(FAKE_MODEL, &["SECOND-OUTPUT"]));
    p.push(Script::text(FAKE_MODEL, &["Pipeline gotowy."]));
    let next = c.turns_continue(sid.clone(), old).await.unwrap();
    until(&mut rx, ends(&next)).await;
    let requests = p.requests();
    assert_eq!(requests.len(), 4, "nie wolno powtórzyć pierwszego etapu");
    assert!(
        requests[2]
            .messages
            .iter()
            .any(|m| m.visible_text().contains("FIRST-OUTPUT"))
    );
    assert!(
        c.turns_list(sid)
            .await
            .unwrap()
            .turns
            .into_iter()
            .find(|t| t.id == next)
            .unwrap()
            .error
            .is_none()
    );
}

#[test]
fn abrupt_runtime_shutdown_recovers_reserved_turn() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let secrets: Arc<dyn accounts_hub_contract::SecretStore> =
        Arc::new(app_core::MemorySecretStore::default());
    let child_secrets = secrets.clone();
    let p = Arc::new(ScriptedProvider::new(Duration::from_millis(30)));
    let child_provider = p.clone();
    let (sid, old, file) = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let mut o = options(
                Some(child_provider.clone()),
                Arc::new(HeadlessShell::default()),
            );
            o.secrets = Some(child_secrets);
            let c = AppCore::build(AppPaths::under(&path), o).await.unwrap();
            c.settings_set(
                "agents.verify_before_done".into(),
                SettingValue::Bool(false),
            )
            .await
            .unwrap();
            let sid = c.sessions_create(SessionTemplate::Empty).await.unwrap().id;
            let folder = c
                .sessions_choose_workdir(sid.clone(), WorkdirChoice::Default)
                .await
                .unwrap()
                .path
                .unwrap();
            let file = std::path::Path::new(&folder).join("crash-once.txt");
            child_provider.push(Script::tool_call(
                FAKE_MODEL,
                "once",
                "fs_write",
                &json!({"path":file,"content":"survived","mode":"create"}),
            ));
            let mut waiting = Script::text(FAKE_MODEL, &["waiting"]);
            waiting
                .steps
                .insert(1, providers_fake::Step::Delay(Duration::from_secs(60)));
            child_provider.push(waiting);
            let old = c
                .turns_send(
                    sid.clone(),
                    SendOptions {
                        parent_id: None,
                        text: "Zapisz i sprawdź".into(),
                        addressed_to: Some("delta".into()),
                        profile: None,
                        attachments: vec![],
                    },
                )
                .await
                .unwrap()
                .assistant_turn_id
                .unwrap();
            tokio::time::timeout(Duration::from_secs(10), async {
                while child_provider.requests().len() < 2 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(std::fs::read_to_string(&file).unwrap(), "survived");
            (sid, old, file)
        })
        // Dropping the reactor aborts the in-flight stream without finalizing its assistant turn.
    })
    .join()
    .unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async{
  let mut o=options(Some(p.clone()),Arc::new(HeadlessShell::default()));o.secrets=Some(secrets);
  let c=AppCore::build(AppPaths::under(dir.path()),o).await.unwrap();
  assert!(!c.turns_list(sid.clone()).await.unwrap().turns.iter().any(|t|t.id==old));
  let runs=c.agents_runs(sid.clone()).await.unwrap();assert!(runs.iter().any(|r|r.run.state==app_core::dto::RunState::Paused));
  p.push(Script::text(FAKE_MODEL,&["Odzyskano po awarii."]));let mut rx=c.subscribe_events();
  let next=c.turns_continue(sid.clone(),old).await.unwrap();until(&mut rx,ends(&next)).await;
  let turns=c.turns_list(sid).await.unwrap().turns;let done=turns.iter().find(|t|t.id==next).unwrap();assert!(done.error.is_none(),"{done:?}");assert!(done.continues.is_none(),"reserved id cannot link to itself");
  assert_eq!(std::fs::read_to_string(file).unwrap(),"survived");assert!(p.requests().last().unwrap().messages.iter().any(|m|m.content.iter().any(|b|matches!(b,providers_contract::ContentBlock::ToolResult(r) if r.tool_use_id=="once"))));
 });
}
