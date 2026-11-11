//! Kolejne delegacje i Krytyczka otrzymują wcześniejsze raporty, także z tej samej tury.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use app_core::dto::{SessionTemplate, SettingValue};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, ends, options, send, until};
use platform_fake::{FakeExec, FakeRun};
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;
use std::{sync::Arc, time::Duration};

/// Dowód wyjścia demonstracyjnej komendy; nie uruchamia PowerShell ani kompilacji na hoście.
fn receipt_exec() -> Arc<FakeExec> {
    let exec = Arc::new(FakeExec::new());
    exec.push(FakeRun::ok(
        "fixture dotnet test TEMP_COPY\nFIXTURE_RESULT_541_PASS_0_FAIL\n",
    ));
    exec
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn critic_receives_previous_delegate_report() {
    let dir = tempfile::tempdir().unwrap();
    let p = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let c = AppCore::build(
        AppPaths::under(dir.path()),
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
    p.push(Script::tool_call(
        FAKE_MODEL,
        "d",
        "delegate_task",
        &json!({"persona":"delta","role":"coder","goal":"Sprawdź kompilację i testy"}),
    ));
    p.push(Script::text(
        FAKE_MODEL,
        &["DELTA-RECEIPT: 541 testów, 0 błędów. Zakres: kopia TEMP, nie oryginał."],
    ));
    p.push(Script::tool_call(FAKE_MODEL,"g","delegate_task",&json!({"persona":"gama","role":"critic","goal":"Zweryfikuj raport Delty; bez wykonywania procesów"})));
    p.push(Script::text(
        FAKE_MODEL,
        &["Zweryfikowałam otrzymany raport."],
    ));
    p.push(Script::text(FAKE_MODEL, &["Podsumowanie."]));
    let mut rx = c.subscribe_events();
    let turn = c
        .turns_send(sid, send("Przegląd programu i weryfikacja", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut rx, ends(&turn)).await;
    let requests = p.requests();
    let critic = &requests[3];
    let context = critic
        .messages
        .iter()
        .map(|m| m.visible_text())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        context.contains("DELTA-RECEIPT"),
        "critic lost previous report"
    );
    assert!(context.contains("541") && context.contains("TEMP"));
    assert!(
        !critic.tools.iter().any(|t| t.name == "shell_run"),
        "handoff must preserve critic's read-only role"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn later_refusal_does_not_erase_recorded_command_output() {
    let dir = tempfile::tempdir().unwrap();
    let p = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let exec = receipt_exec();
    let mut opts = options(Some(p.clone()), Arc::new(HeadlessShell::default()));
    opts.exec = Some(exec.clone());
    let c = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    c.settings_set(
        "agents.verify_before_done".into(),
        SettingValue::Bool(false),
    )
    .await
    .unwrap();
    let sid = c.sessions_create(SessionTemplate::Empty).await.unwrap().id;
    p.push(Script::tool_call(FAKE_MODEL,"d","delegate_task",&json!({"persona":"delta","role":"coder","goal":"Uruchom demonstracyjny zapis wyniku, bez rzeczywistej kompilacji"})));
    p.push(Script::tool_call(FAKE_MODEL,"receipt","shell_run",&json!({"command":"Write-Output 'fixture dotnet test TEMP_COPY'; Write-Output 'FIXTURE_RESULT_541_PASS_0_FAIL'","shell":"powershell"})));
    p.push(Script::refusal(FAKE_MODEL));
    p.push(Script::tool_call(
        FAKE_MODEL,
        "g",
        "delegate_task",
        &json!({"persona":"gama","role":"critic","goal":"Sprawdź zachowane wyniki, tylko odczyt"}),
    ));
    p.push(Script::text(
        FAKE_MODEL,
        &["Otrzymałam zapis demonstracyjnej komendy."],
    ));
    p.push(Script::text(FAKE_MODEL, &["Raport końcowy."]));
    let mut rx = c.subscribe_events();
    let turn = c
        .turns_send(sid.clone(), send("Sprawdź przekazywanie wyników", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut rx, ends(&turn)).await;
    assert_eq!(exec.runs().len(), 1, "jedna demonstracyjna komenda");
    let requests = p.requests();
    let critic = &requests[4];
    let context = critic
        .messages
        .iter()
        .map(|m| m.visible_text())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(context.contains("FIXTURE_RESULT_541_PASS_0_FAIL"));
    assert!(context.contains("nieukończone"));
    assert!(!critic.tools.iter().any(|t| t.name == "shell_run"));
    let final_turn = c
        .turns_list(sid)
        .await
        .unwrap()
        .turns
        .into_iter()
        .find(|t| t.id == turn)
        .unwrap();
    assert!(
        final_turn
            .text
            .contains("Zapis poleceń związanych z kompilacją i testami")
    );
    assert!(final_turn.text.contains("FIXTURE_RESULT_541_PASS_0_FAIL"));
    assert!(
        final_turn.text.contains("Write-Output"),
        "fixture is output evidence, not proof that dotnet tests ran"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn same_model_turn_delegations_forward_completed_report() {
    use providers_contract::{ProviderEvent, StopReason, ToolArguments};
    use providers_fake::Step;
    let dir = tempfile::tempdir().unwrap();
    let p = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let c = AppCore::build(
        AppPaths::under(dir.path()),
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
    let mut steps = Vec::new();
    for (index, id, persona, role) in [(0, "d", "delta", "coder"), (1, "g", "gama", "critic")] {
        steps.push(Step::Emit(ProviderEvent::ToolCallStart {
            index,
            id: id.into(),
            name: "delegate_task".into(),
        }));
        steps.push(Step::Emit(ProviderEvent::ToolCallEnd{index,id:id.into(),arguments:ToolArguments::from_raw(&json!({"persona":persona,"role":role,"goal":"Wykonaj swoją część i uwzględnij poprzednie wyniki"}).to_string())}));
    }
    steps.push(Step::Emit(ProviderEvent::stop(StopReason::ToolUse)));
    p.push(Script::new(steps));
    p.push(Script::text(FAKE_MODEL, &["SAME-BATCH-RECEIPT 541 TEMP"]));
    p.push(Script::text(FAKE_MODEL, &["Zweryfikowane"]));
    p.push(Script::text(FAKE_MODEL, &["Koniec"]));
    let mut rx = c.subscribe_events();
    let turn = c
        .turns_send(sid, send("Dwie agentki po kolei", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut rx, ends(&turn)).await;
    let req = p.requests();
    let context = req[2]
        .messages
        .iter()
        .map(|m| m.visible_text())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(context.contains("SAME-BATCH-RECEIPT"));
}

#[test]
fn receipts_survive_limit_and_core_restart_without_repeating_delegate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let p = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let worker_provider = p.clone();
    let secrets: Arc<dyn accounts_hub_contract::SecretStore> =
        Arc::new(app_core::MemorySecretStore::default());
    let worker_secrets = secrets.clone();
    let exec = receipt_exec();
    let worker_exec = exec.clone();
    let (sid,old)=std::thread::spawn(move||{
  let runtime=tokio::runtime::Runtime::new().unwrap();runtime.block_on(async move{
   let mut opts=options(Some(worker_provider.clone()),Arc::new(HeadlessShell::default()));opts.secrets=Some(worker_secrets);opts.exec=Some(worker_exec);let c=AppCore::build(AppPaths::under(&path),opts).await.unwrap();
   c.settings_set("agents.max_steps".into(),SettingValue::Number(5.into())).await.unwrap();c.settings_set("agents.verify_before_done".into(),SettingValue::Bool(false)).await.unwrap();let sid=c.sessions_create(SessionTemplate::Empty).await.unwrap().id;
   worker_provider.push(Script::tool_call(FAKE_MODEL,"d","delegate_task",&json!({"persona":"delta","role":"coder","goal":"Uruchom demonstracyjny zapis wyniku bez kompilacji"})));
   worker_provider.push(Script::tool_call(FAKE_MODEL,"receipt","shell_run",&json!({"command":"Write-Output 'fixture dotnet test TEMP_COPY'; Write-Output 'FIXTURE_RESULT_541_PASS_0_FAIL'","shell":"powershell"})));worker_provider.push(Script::refusal(FAKE_MODEL));
   let mut rx=c.subscribe_events();let turn=c.turns_send(sid.clone(),send("Sprawdź przekazywanie wyników",None)).await.unwrap().assistant_turn_id.unwrap();until(&mut rx,ends(&turn)).await;
   let paused=c.turns_list(sid.clone()).await.unwrap().turns.into_iter().find(|t|t.id==turn).unwrap();assert_eq!(paused.error.unwrap().code,app_core::dto::TurnErrorCode::TaskPaused);assert!(paused.text.contains("FIXTURE_RESULT_541_PASS_0_FAIL"));(sid,turn)
  })
  // Zakończenie całego reaktora zwalnia dziennik, jak przy wyjściu z aplikacji.
 }).join().unwrap();
    assert_eq!(exec.runs().len(), 1, "jedna komenda przed restartem");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async{
  let mut opts=options(Some(p.clone()),Arc::new(HeadlessShell::default()));opts.secrets=Some(secrets);opts.exec=Some(exec.clone());let c=AppCore::build(AppPaths::under(dir.path()),opts).await.unwrap();
  p.push(Script::tool_call(FAKE_MODEL,"g","delegate_task",&json!({"persona":"gama","role":"critic","goal":"Sprawdź zachowane wyniki, tylko odczyt"})));p.push(Script::text(FAKE_MODEL,&["Otrzymałam zapis poprzedniej komendy"]));p.push(Script::text(FAKE_MODEL,&["Raport końcowy"]));
  let mut rx=c.subscribe_events();let turn=c.turns_continue(sid.clone(),old).await.unwrap();until(&mut rx,ends(&turn)).await;assert_eq!(exec.runs().len(),1,"restart nie powtarza wykonanej komendy");let requests=p.requests();assert_eq!(requests.len(),6,"restart must not rerun completed delegate");
  let critic=&requests[4];assert!(critic.system.as_deref().unwrap_or("").contains("Gama"));let context=critic.messages.iter().map(|m|m.visible_text()).collect::<Vec<_>>().join("\n");assert!(context.contains("FIXTURE_RESULT_541_PASS_0_FAIL"));assert!(context.contains("nieukończone"));assert!(!critic.tools.iter().any(|t|t.name=="shell_run"));
  let result=c.turns_list(sid).await.unwrap().turns.into_iter().find(|t|t.id==turn).unwrap();assert!(result.error.is_none(),"{result:?}");assert!(result.text.contains("Zapis poleceń związanych z kompilacją i testami"));assert!(result.text.contains("FIXTURE_RESULT_541_PASS_0_FAIL"));
 });
}
