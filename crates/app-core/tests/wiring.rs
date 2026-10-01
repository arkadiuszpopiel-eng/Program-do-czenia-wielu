//! Moduły podpięte w kompozycji: cofnięcie kroku dziennika `undo-journal`, kill-switch
//! anulujący trwające generacje, poziomy autonomii przez Brokera (bez Broker-UI — odmowa
//! podniesienia), czytanie na głos na atrapach (`voice-tts-fake`, `voice-audio-fake`),
//! `updater::mark_good` po zdrowym starcie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use app_core::dto::{AlfaEvent, AutonomyLevel, BrokerIntentStatus, SessionTemplate, StopReason};
use app_core::ports::{ApprovalWindow, HeadlessShell, KillOrigin};
use app_core::{AppCore, AppError, AppPaths, ErrorCode};
use common::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_step_restores_file_and_checks_session() {
    let h = harness().await;
    let sid = h
        .core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let journal = h.core.undo_journal().expect("undo-journal podpięty");
    let file = h.dir.path().join("raport.txt");
    std::fs::write(&file, b"stara wersja").unwrap();
    let step = journal
        .begin_step(undo_journal_contract::StepCtx::new(
            &sid,
            Some("delta"),
            "Edycja raportu",
        ))
        .unwrap();
    journal.write(step, &file, b"nowa wersja").unwrap();
    journal.commit_step(step).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"nowa wersja");

    let other = h
        .core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let foreign = h
        .core
        .turns_undo_step(app_core::ids::undo_dto(&other.as_str().into(), step.0))
        .await;
    assert_eq!(foreign.unwrap_err().code, ErrorCode::NotFound);
    let token = app_core::ids::undo_dto(&sid.as_str().into(), step.0);
    h.core.turns_undo_step(token.clone()).await.unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"stara wersja");
    let timeline = h.core.timeline_list(sid, all_events()).await.unwrap();
    assert!(
        timeline.iter().any(|e| e.title.starts_with("Cofnięto: ")),
        "{timeline:?}"
    );
    // Drugie cofnięcie tego samego kroku — czytelny błąd.
    assert!(h.core.turns_undo_step(token).await.is_err());
}

fn all_events() -> app_core::dto::TimelineFilter {
    serde_json::from_value(serde_json::json!({
        "kinds": ["model_call", "tool", "audit", "ui", "voice", "diagnostics"],
        "min_level": "trace",
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kill_switch_cancels_running_generations_quickly() {
    let mut h = harness_with(Duration::from_millis(40), true).await;
    let mut turns = Vec::new();
    for i in 0..3 {
        let sid = h
            .core
            .sessions_create(SessionTemplate::Empty)
            .await
            .unwrap()
            .id;
        let long = format!("{i} {}", "słowo ".repeat(200));
        let sent = h.core.turns_send(sid, send(&long, None)).await.unwrap();
        turns.push(sent.assistant_turn_id.unwrap());
    }
    until(&mut h.rx, |e| matches!(e, AlfaEvent::TextDelta { .. })).await;
    let started = Instant::now();
    let stopped = h.core.system_kill_all(KillOrigin::Hotkey).await;
    let took = started.elapsed();
    let limit = budget(200);
    eprintln!("kill-switch: {stopped} generacji w {took:?} (budżet {limit:?})");
    assert_eq!(stopped, 3);
    assert!(took <= limit, "{took:?} > {limit:?}");
    let mut cancelled = 0;
    while cancelled < 3 {
        let events = until(&mut h.rx, |e| matches!(e, AlfaEvent::Stop { .. })).await;
        cancelled += turns
            .iter()
            .filter(|t| stop_reason(&events, t) == Some(StopReason::Cancelled))
            .count();
    }
    let audit =
        std::fs::read_to_string(h.dir.path().join("local/broker-dev/audit.ndjson")).unwrap();
    assert!(
        audit.contains("broker.kill_switch"),
        "kill-switch Brokera w Audycie"
    );
}

/// Atrapa okna Brokera: zapamiętuje pokazane prośby.
#[derive(Default)]
struct Window(Mutex<Vec<String>>);

impl ApprovalWindow for Window {
    fn present(&self, approval: &str) -> Result<(), AppError> {
        self.0.lock().unwrap().push(approval.to_owned());
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn autonomy_changes_go_through_broker() {
    let h = harness().await;
    let core = &h.core;
    assert_eq!(
        core.permissions_get(None).await.unwrap().global,
        AutonomyLevel::L3
    );
    let denied = core
        .permissions_request_level(AutonomyLevel::L4, None)
        .await
        .unwrap_err();
    assert_eq!(denied.message, app_core::ports::NEEDS_BROKER_WINDOW);
    assert_eq!(
        core.permissions_get(None).await.unwrap().global,
        AutonomyLevel::L3
    );
    let lowered = core
        .permissions_request_level(AutonomyLevel::L1, None)
        .await
        .unwrap();
    assert_eq!(lowered.status, BrokerIntentStatus::Applied);
    assert_eq!(
        core.permissions_get(None).await.unwrap().global,
        AutonomyLevel::L1
    );
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    assert_eq!(
        core.sessions_list().await.unwrap()[0].autonomy,
        AutonomyLevel::L1
    );
    // Sesja nie może podnieść się ponad poziom bez zgody; obniżenie — od razu.
    assert!(
        core.permissions_request_level(AutonomyLevel::L2, Some(sid.clone()))
            .await
            .is_err()
    );
    core.permissions_request_level(AutonomyLevel::L0, Some(sid.clone()))
        .await
        .unwrap();
    let state = core.permissions_get(Some(sid.clone())).await.unwrap();
    assert_eq!(
        (state.global, state.session),
        (AutonomyLevel::L1, Some(AutonomyLevel::L0))
    );

    // Z oknem Brokera: prośba trafia do karty (zatwierdza tylko Broker-UI).
    let dir = tempfile::tempdir().unwrap();
    let window = Arc::new(Window::default());
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.approval_window = Some(window.clone());
    let with_ui = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let opened = with_ui
        .permissions_request_level(AutonomyLevel::L4, None)
        .await
        .unwrap();
    assert_eq!(opened.status, BrokerIntentStatus::OpenedBroker);
    assert_eq!(
        window.0.lock().unwrap().as_slice(),
        std::slice::from_ref(&opened.request_id)
    );
    assert_eq!(
        with_ui.permissions_get(None).await.unwrap().global,
        AutonomyLevel::L3
    );
    with_ui
        .permissions_open_approval(opened.request_id)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_aloud_uses_tts_and_audio_on_fakes() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let audio = voice_audio_fake::FakeAudio::new();
    let mut opts = options(Some(provider), Arc::new(HeadlessShell::default()));
    opts.audio = Some(Arc::new(audio.clone()));
    opts.tts = Some(Arc::new(voice_tts_fake::FakeTts::new()));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut rx = core.subscribe_events();
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let turn = core
        .turns_send(sid, send("Przeczytaj to", None))
        .await
        .unwrap();
    let answer = turn.assistant_turn_id.unwrap();
    until(&mut rx, ends(&answer)).await;
    core.turns_read_aloud(answer.clone()).await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while audio.recorded_output().iter().all(|s| s.abs() < 1e-4) {
        assert!(Instant::now() < deadline, "brak dźwięku na wyjściu");
        audio.advance(Duration::from_millis(50));
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    core.voice_stop_speech().await.unwrap();

    // Bez silnika TTS — czytelny komunikat.
    let h = harness().await;
    let sid = h
        .core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let mut rx = h.rx;
    let turn = h.core.turns_send(sid, send("Cześć", None)).await.unwrap();
    let answer = turn.assistant_turn_id.unwrap();
    until(&mut rx, ends(&answer)).await;
    let err = h.core.turns_read_aloud(answer).await.unwrap_err();
    assert_eq!(err.message, app_core::NO_TTS);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthy_start_marks_pending_version_good() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::under(dir.path());
    let layout = updater_contract::Layout::new(&paths.local);
    let version = semver::Version::new(1, 2, 3);
    let mut state = updater_contract::CurrentState::initial(version.clone(), chrono::Utc::now());
    state.pending = true;
    std::fs::create_dir_all(&paths.local).unwrap();
    std::fs::write(&layout.current, serde_json::to_vec(&state).unwrap()).unwrap();
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.app_version = version.to_string();
    opts.healthy_after = Duration::from_millis(50);
    let _core = AppCore::build(paths, opts).await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let now: updater_contract::CurrentState =
            serde_json::from_slice(&std::fs::read(&layout.current).unwrap()).unwrap();
        if !now.pending {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "wersja nie została oznaczona jako dobra"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
