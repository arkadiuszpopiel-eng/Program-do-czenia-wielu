//! #58: prawdziwy odczyt TEMP → odbudowany Core → pytanie, najpierw bez prawdziwego modelu.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
#[path = "support/executed_read.rs"]
mod executed_read;
#[path = "support/history_seed.rs"]
mod history_seed;
#[path = "support/local_probe_brain.rs"]
mod local_probe_brain;
#[path = "jarvis_live/model.rs"]
mod probe_model;

use app_core::{AppCore, AppPaths};
use common::{ends, send, until};
use providers_fake::{FAKE_MODEL, Script};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn actual_read_survives_restart_and_disabled_workdir_before_next_question() {
    let (mut a, secrets) = executed_read::fixture().await;
    drop(a.h.core);
    let mut opts = common::options(Some(a.h.provider.clone()), a.h.shell.clone());
    opts.secrets = Some(secrets);
    opts.exec = Some(a.exec.clone());
    a.h.core = AppCore::build(AppPaths::under(a.h.dir.path()), opts)
        .await
        .unwrap();
    a.h.rx = a.h.core.subscribe_events();
    a.h.provider.push(Script::text(
        FAKE_MODEL,
        &["Kontrola wejścia; odpowiedź testowa."],
    ));
    let mut goal = send(history_seed::QUERY, None);
    goal.addressed_to = Some("delta".into());
    let turn =
        a.h.core
            .turns_send(a.sid.clone(), goal)
            .await
            .unwrap()
            .assistant_turn_id
            .unwrap();
    until(&mut a.h.rx, ends(&turn)).await;
    let request = a.h.provider.requests().last().cloned().unwrap();
    assert_eq!(
        request.messages.last().unwrap().visible_text(),
        history_seed::QUERY
    );
    assert!(
        executed_read::checked_records(&request).is_some(),
        "brak autentycznego Replay/fs_read w kolejnym żądaniu"
    );
    let turns = a.h.core.turns_list(a.sid).await.unwrap();
    assert!(
        turns
            .turns
            .iter()
            .find(|t| t.id == turn)
            .unwrap()
            .tools
            .is_empty()
    );
    if let Some(path) = std::env::var_os("ALFA_ACTUAL_READ_CAPTURE") {
        std::fs::write(path, serde_json::to_vec_pretty(&request).unwrap()).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "prawdziwy modelCPU po rzeczywistym fs_read TEMP; ALFA_ACTUAL_READ_REPORT i ALFA_LIVE_*"]
async fn capture_actual_read_on_local_model() {
    use local_probe_brain::{BoundedProvider, LocalBrain};
    use providers_local_impl::{LocalProvider, Sidecar, TokioLauncher};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    let report_path = std::env::var("ALFA_ACTUAL_READ_REPORT").expect("osobny raport");
    let (mut a, secrets) = executed_read::fixture().await;
    drop(a.h.core);
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
    let mut opts = common::options(None, a.h.shell.clone());
    opts.secrets = Some(secrets);
    opts.exec = Some(a.exec.clone());
    opts.brain = Some(brain.clone());
    a.h.core = AppCore::build(AppPaths::under(a.h.dir.path()), opts)
        .await
        .unwrap();
    let mut events = a.h.core.subscribe_events();
    let mut goal = send(history_seed::QUERY, None);
    goal.addressed_to = Some("delta".into());
    let started = Instant::now();
    let turn =
        a.h.core
            .turns_send(a.sid.clone(), goal)
            .await
            .unwrap()
            .assistant_turn_id
            .unwrap();
    let (finished, first_text_ms) = wait_live(&mut events, &turn, started).await;
    if !finished {
        a.h.core.turns_stop(a.sid.clone()).await.unwrap();
    }
    let turns = a.h.core.turns_list(a.sid).await.unwrap();
    let answer = turns.turns.iter().find(|t| t.id == turn).unwrap();
    let request = brain.sent.lock().unwrap().last().cloned();
    let facts = request.as_ref().and_then(executed_read::checked_records);
    let input_ok = facts.is_some()
        && request
            .as_ref()
            .is_some_and(|r| r.messages.last().unwrap().visible_text() == history_seed::QUERY);
    let transport_ok = finished
        && answer.error.is_none()
        && !answer.text.trim().is_empty()
        && answer.tools.is_empty()
        && answer.author == "delta";
    let report = serde_json::json!({"format":"alfa-actual-read-live-v1","platform":std::env::consts::OS,
        "scope":"Core+native TEMP fs_read (scripted tool choice), then rebuilt Core+LocalProvider CPU; no GUI/mic/GPU/API/CLI or owner files",
        "model":model,"agent":"delta","mode":"standard","context":8192,"max_tokens":96,"temperature":0.0,
        "prior_action":"native fs_read returned fresh random TEMP content; persisted Replay checked; file contents not forwarded",
        "current_turn_tools":answer.tools.len(),"input_ok":input_ok,"transport_ok":transport_ok,
        "elapsed_ms":started.elapsed().as_millis(),"first_text_core_ms":first_text_ms,"request":request,"records":facts,
        "text":answer.text,"error":answer.error,"truncated":answer.truncated,"usage":answer.usage,
        "semantic_acceptance":if transport_ok {"manual review required; transport is not fidelity acceptance"} else {"NOT_EVALUATED"}});
    std::fs::write(report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    sidecar
        .stop("koniec rzeczywistej kontroli odczytu; bez ponowienia")
        .await;
    assert!(input_ok, "brak rzeczywistego Replay w wejściu");
    assert!(transport_ok, "błąd transportu; surowy raport zachowany");
}

async fn wait_live(
    events: &mut tokio::sync::broadcast::Receiver<app_core::EventBatch>,
    turn: &str,
    started: std::time::Instant,
) -> (bool, Option<u128>) {
    use app_core::dto::AlfaEvent;
    let mut first = None;
    let done=tokio::time::timeout(std::time::Duration::from_secs(120),async{
        loop {match events.recv().await {
            Ok(frame)=>{
                for e in frame.iter(){if let AlfaEvent::TextDelta{turn_id,text,..}=e && turn_id==turn && !text.trim().is_empty() && first.is_none(){first=Some(started.elapsed().as_millis());}}
                if frame.iter().any(|e|matches!(e,AlfaEvent::Stop{turn_id,..}|AlfaEvent::Error{turn_id,..}if turn_id==turn)){return true;}
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_))=>{}
            Err(_)=>return false,
        }}
    }).await.unwrap_or(false);
    (done, first)
}
