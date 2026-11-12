//! Ręczny ekran #58: prawdziwy Core i LocalProvider, syntetyczna historia, bez akcji komputera.
//! ALFA_LIVE_SERVER/MODELS_DIR/MODEL jak jarvis_live; ALFA_RECORDS_LIVE_REPORT — osobny plik.
//! ALFA_RECORDS_LIVE_CASES=3 (ekran) lub30 (macierz). Wynik testu sprawdza transport/wejście,
//! nie zastępuje przeglądu semantyki zapisanych odpowiedzi ani odbioru sprzętu.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
#[path = "support/history_seed.rs"]
mod history_seed;
#[path = "support/local_probe_brain.rs"]
mod local_probe_brain;
#[path = "jarvis_live/model.rs"]
mod probe_model;

use app_core::dto::{AgentDraft, AlfaEvent, SettingValue};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths, EventBatch};
use local_probe_brain::{BoundedProvider, LocalBrain};
use providers_contract::{ChatRequest, Role};
use providers_local_impl::{LocalProvider, Sidecar, TokioLauncher};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MARK: &str = "[Zapis czynności aplikacji]";

async fn register_luna(core: &AppCore) -> String {
    let draft: AgentDraft = serde_json::from_value(json!({
        "name":"Luna", "glyph":"L", "color":"color.agent.custom-2",
        "character":"Spokojna i konkretna.",
        "role":{"id":"rozmowczyni", "name":"Rozmówczyni", "description":"Rozmawia.",
                "prompt":"Odpowiadam po polsku.", "model_policy":"conversation", "tools":[],
                "read_only":true, "untrusted_isolated":false, "author":false},
        "limits":{"autonomy":"L1", "memory_scope":"agent", "retain_days":30}
    }))
    .unwrap();
    let dry = core.builder_dry_run(draft.clone()).await.unwrap();
    assert!(dry.passed);
    core.builder_save(draft, dry.hash).await.unwrap().role
}

fn input_ok(request: &ChatRequest, scenario: &str) -> bool {
    let data: Vec<_> = request
        .messages
        .iter()
        .filter_map(|m| {
            let text = m.visible_text();
            let value = text.strip_prefix(MARK)?;
            (m.role == Role::User)
                .then(|| serde_json::from_str::<Value>(value.trim()).ok())
                .flatten()
        })
        .collect();
    request.tools.is_empty()
        && request.params.max_tokens == Some(96)
        && request
            .messages
            .last()
            .is_some_and(|m| m.visible_text() == history_seed::QUERY)
        && data.len() == 1
        && data[0]["counts"]["recorded_done"] == u64::from(scenario == "recorded_done")
        && data[0]["counts"]["needs_confirmation"] == u64::from(scenario == "needs_confirmation")
        && !serde_json::to_string(request)
            .unwrap()
            .contains("sekret-testowy")
}

async fn wait_for_turn(
    events: &mut tokio::sync::broadcast::Receiver<EventBatch>,
    turn: &str,
    started: Instant,
) -> (bool, Option<u128>) {
    let mut first_text_ms = None;
    let finished = tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            match events.recv().await {
                Ok(batch) => {
                    for event in batch.iter() {
                        if let AlfaEvent::TextDelta { turn_id, text, .. } = event
                            && turn_id == turn
                            && !text.trim().is_empty()
                            && first_text_ms.is_none()
                        {
                            first_text_ms = Some(started.elapsed().as_millis());
                        }
                    }
                    if batch.iter().any(|e| {
                        matches!(e, AlfaEvent::Stop { turn_id, .. }
                        | AlfaEvent::Error { turn_id, .. } if turn_id == turn)
                    }) {
                        return true;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => return false,
            }
        }
    })
    .await
    .unwrap_or(false);
    (finished, first_text_ms)
}

#[tokio::test]
async fn closed_stream_is_not_a_completed_turn() {
    let (sender, mut receiver) = tokio::sync::broadcast::channel(1);
    drop(sender);
    assert!(
        !wait_for_turn(&mut receiver, "absent", Instant::now())
            .await
            .0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "prawdziwy lokalny8B; ALFA_LIVE_*/ALFA_RECORDS_LIVE_*, zapis do przeglądu semantyki"]
async fn capture_branch_records_on_local_model() {
    let limit: usize = std::env::var("ALFA_RECORDS_LIVE_CASES")
        .unwrap_or_else(|_| "3".into())
        .parse()
        .unwrap();
    assert!(matches!(limit, 3 | 30), "tylko ekran3 albo pełna macierz30");
    let report_path = std::env::var("ALFA_RECORDS_LIVE_REPORT").expect("osobny raport");
    let (models, config, model) = probe_model::configuration();
    let sidecar = Sidecar::new(config, Arc::new(TokioLauncher::default()), None, None).unwrap();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let brain = Arc::new(LocalBrain {
        provider: Arc::new(BoundedProvider(
            LocalProvider::new(models, sidecar.clone()),
            sent.clone(),
        )),
        model: model.clone(),
        seen: Mutex::new(Vec::new()),
        sent,
    });
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::under(dir.path());
    let mut opts = common::options(None, Arc::new(HeadlessShell::default()));
    let secrets = opts.secrets.clone().unwrap();
    opts.brain = Some(brain.clone());
    let core = AppCore::build(paths.clone(), opts).await.unwrap();
    let role = register_luna(&core).await;
    let cases: Vec<_> = ["alfa", "beta", "gama", "delta", "luna"]
        .into_iter()
        .flat_map(|agent| {
            ["standard", "jarvis"].into_iter().flat_map(move |mode| {
                ["empty_record", "recorded_done", "needs_confirmation"]
                    .into_iter()
                    .map(move |scenario| (agent, mode, scenario))
            })
        })
        .collect();
    let mut events = core.subscribe_events();
    let mut rows = Vec::new();
    for (agent, mode, scenario) in cases.iter().take(limit) {
        core.settings_set(
            "agents.interaction_mode".into(),
            SettingValue::Text((*mode).into()),
        )
        .await
        .unwrap();
        let sid = history_seed::seed(&core, &paths, secrets.clone(), agent, scenario).await;
        if *agent == "luna" {
            core.agents_set_roles(sid.clone(), "luna".into(), vec![role.clone()])
                .await
                .unwrap();
        }
        let mut goal = common::send(history_seed::QUERY, None);
        goal.addressed_to = Some((*agent).into());
        let started = Instant::now();
        let turn = core
            .turns_send(sid.clone(), goal)
            .await
            .unwrap()
            .assistant_turn_id
            .unwrap();
        let (finished, first_text_ms) = wait_for_turn(&mut events, &turn, started).await;
        if !finished {
            core.turns_stop(sid.clone()).await.unwrap();
        }
        let turns = core.turns_list(sid).await.unwrap();
        let answer = turns.turns.iter().find(|t| t.id == turn).unwrap();
        let request = brain.sent.lock().unwrap().last().cloned();
        let correct_input = request.as_ref().is_some_and(|r| input_ok(r, scenario));
        let transport_ok = finished
            && answer.error.is_none()
            && !answer.text.trim().is_empty()
            && answer.tools.is_empty()
            && answer.author == *agent;
        rows.push(json!({"agent":agent,"mode":mode,"scenario":scenario,
            "elapsed_ms":started.elapsed().as_millis(),"first_text_core_ms":first_text_ms,
            "text":answer.text,"error":answer.error,"truncated":answer.truncated,
            "usage":answer.usage,"response_author":answer.author,"actual_tools":answer.tools.len(),
            "request":request,"input_ok":correct_input,"transport_ok":transport_ok}));
        let report = json!({"format":"alfa-records-live-v1","model":model,
            "platform":std::env::consts::OS,"scope":"Core+LocalProvider CPU; history synthetic; no desktop/mic/GPU/API or business actions",
            "interpretation":"transport/input checks only; semantic review of raw text required",
            "max_tokens":96,"temperature":0.0,"context":8192,"turn_budget_s":120,
            "requested_cases":limit,"results":rows,"not_run":&cases[rows.len()..],
            "state":if !transport_ok || !correct_input {"failed_transport_or_input"}
                else if rows.len()==limit {"completed"} else {"running"}});
        std::fs::write(&report_path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
        eprintln!("{agent}/{mode}/{scenario}: transport={transport_ok},input={correct_input}");
        if !transport_ok || !correct_input {
            break;
        }
    }
    sidecar.stop("koniec ekranu zapisów; bez ponowień").await;
    assert_eq!(rows.len(), limit, "nie wykonano całego żądanego ekranu");
    assert!(
        rows.iter()
            .all(|r| r["transport_ok"] == true && r["input_ok"] == true),
        "transport lub wejście niezaliczone; sprawdź osobny raport"
    );
}
