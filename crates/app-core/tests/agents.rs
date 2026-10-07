//! Agentka z narzędziami w aplikacji (na atrapach dostawcy i wykonania poleceń): zadanie na
//! plikach → krok w Replay → „Cofnij" przywraca stan; odmowa Brokera → agentka dostaje powód;
//! prośba o zgodę bez decyzji → odmowa po czasie; kill-switch zatrzymuje pętlę i polecenie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::{Duration, Instant};

use app_core::ErrorCode;
use app_core::dto::{AlfaEvent, ApprovalStatus, ReplayStatus, RunState, StopReason};
use app_core::ports::KillOrigin;
use common::agents::*;
use common::*;
use platform_fake::FakeRun;
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn file_task_shows_in_replay_and_undo_restores_state() {
    let mut a = agents().await;
    // Argumenty ASCII: `Script::tool_call` dzieli JSON w połowie bajtów (providers-fake).
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "c1",
        "fs_write",
        &json!({ "path": "notatka.txt", "content": "Mleko, chleb, jajka" }),
    ));
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Zapisałam notatkę."]));
    let (turn, events) = run_turn(&mut a, "Delta, zapisz notatkę z listą zakupów.").await;
    let file = a.workdir.join("notatka.txt");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "Mleko, chleb, jajka"
    );
    assert_eq!(stop_reason(&events, &turn), Some(StopReason::End));

    let run = last_run(&a).await;
    assert_eq!(run.run.state, RunState::Completed, "{run:#?}");
    assert_eq!(run.run.agent, "delta");
    let step = run
        .steps
        .iter()
        .find(|s| s.tool.as_deref() == Some("fs_write"))
        .expect("krok fs_write w Replay");
    assert_eq!(step.status, ReplayStatus::Ok);
    let token = step.undo_token.clone().expect("krok cofalny");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AlfaEvent::AgentStep { .. }))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        AlfaEvent::ToolCall { step, .. } if step.undo_token.as_deref() == Some(token.as_str())
    )));
    // Tura agentki: odpowiedź końcowa + kroki narzędzi w faktach (po ponownym wczytaniu też).
    let snapshot = a.h.core.turns_list(a.sid.clone()).await.unwrap();
    let reply = snapshot.turns.iter().find(|t| t.id == turn).unwrap();
    assert_eq!(reply.text, "Zapisałam notatkę.");
    assert!(
        reply
            .tools
            .iter()
            .any(|t| t.undo_token == Some(token.clone()))
    );

    a.h.core.turns_undo_step(token.clone()).await.unwrap();
    assert!(!file.exists(), "„Cofnij” usuwa utworzony plik");
    let run = last_run(&a).await;
    let step = run
        .steps
        .iter()
        .find(|s| s.undo_token == Some(token.clone()));
    assert!(step.is_some_and(|s| s.undone), "{run:#?}");
    let again = a.h.core.turns_undo_step(token).await.unwrap_err();
    assert_ne!(again.code, ErrorCode::Internal, "{again:?}");
}

/// Karta „Cofnij” zapisu zmiennej (`system_env_set`, token `"<sesja>:v<krok>"`): cofa tylko krok
/// przebiegu tej sesji (cudzy albo nieznany — odmowa bez skutku), błędny rodzaj tokenu — błąd
/// danych. Pełna ścieżka zapis → cofnięcie wymaga zgody w oknie Brokera (test w `app-agents`).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn env_undo_token_only_for_steps_of_this_session() {
    let a = agents().await;
    let token = app_core::ids::undo_env_dto(&a.sid.as_str().into(), 1);
    let err = a.h.core.turns_undo_step(token).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound, "{err:?}");
    let bad = a.h.core.turns_undo_step(format!("{}:x1", a.sid)).await;
    assert_eq!(bad.unwrap_err().code, ErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn broker_denial_reason_reaches_the_agent() {
    let mut a = agents().await;
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "c1",
        "shell_run",
        &json!({ "command": "bcdedit /set {current} safeboot minimal" }),
    ));
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Nie mogę tego zrobić."]));
    let _ = run_turn(&mut a, "Delta, włącz tryb awaryjny.").await;
    assert!(
        a.exec.runs().is_empty(),
        "polecenie nie zostało uruchomione"
    );
    let run = last_run(&a).await;
    let step = run
        .steps
        .iter()
        .find(|s| s.tool.as_deref() == Some("shell_run"))
        .unwrap();
    assert_eq!(step.status, ReplayStatus::Denied, "{step:#?}");
    let requests = a.h.provider.requests();
    let seen = tool_results(requests.last().unwrap());
    assert!(seen.contains("Odmowa"), "{seen}");
    assert!(
        seen.contains("Jądr"),
        "powód blokady w wyniku narzędzia: {seen}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn approval_without_decision_times_out_as_denial() {
    let mut a = agents().await;
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "c1",
        "shell_run",
        &json!({ "command": "curl https://example.com/raport.csv -o raport.csv" }),
    ));
    a.h.provider.push(Script::text(
        FAKE_MODEL,
        &["Brak zgody — nie pobrałam pliku."],
    ));
    let started = Instant::now();
    let (_, events) = run_turn(&mut a, "Delta, pobierz raport.").await;
    let waited = started.elapsed();
    let card = events.iter().find_map(|e| match e {
        AlfaEvent::ApprovalPending { approval, .. } => Some(approval.clone()),
        _ => None,
    });
    let card = card.expect("karta „czeka na zatwierdzenie”");
    assert!(!card.broker_window, "bez Broker-UI karta wyjaśnia stan");
    assert!(card.expires_at.is_some());
    assert!(events.iter().any(|e| matches!(
        e,
        AlfaEvent::AgentRunUpdated { run, .. } if run.state == RunState::WaitingApproval
    )));
    let last = events.iter().rev().find_map(|e| match e {
        AlfaEvent::ApprovalPending { approval, .. } => Some(approval.status),
        _ => None,
    });
    assert_ne!(last, Some(ApprovalStatus::Pending), "karta rozstrzygnięta");
    assert!(waited >= Duration::from_millis(900), "{waited:?}");
    assert!(a.exec.runs().is_empty());
    let requests = a.h.provider.requests();
    let seen = tool_results(requests.last().unwrap());
    assert!(seen.contains("Odmowa"), "{seen}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kill_switch_stops_agent_loop_and_running_command() {
    let mut a = agents().await;
    a.exec.on_command("Start-Sleep", FakeRun::hanging());
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "c1",
        "shell_run",
        &json!({ "command": "Start-Sleep -Seconds 600" }),
    ));
    let sent =
        a.h.core
            .turns_send(a.sid.clone(), to_delta("Delta, poczekaj 10 minut."))
            .await
            .unwrap();
    let turn = sent.assistant_turn_id.unwrap();
    until(&mut a.h.rx, |e| {
        matches!(e, AlfaEvent::AgentStep { step, .. }
            if step.tool.as_deref() == Some("shell_run") && step.status == ReplayStatus::Running)
    })
    .await;
    // Polecenie wystartowało w atrapie (Job Object), zanim zatrzymamy wszystko.
    let deadline = Instant::now() + Duration::from_secs(10);
    while a.exec.runs().is_empty() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(a.exec.runs().len(), 1);
    let started = Instant::now();
    let stopped = a.h.core.system_kill_all(KillOrigin::Hotkey).await;
    assert!(stopped >= 1);
    let events = until(&mut a.h.rx, ends(&turn)).await;
    let took = started.elapsed();
    assert!(took <= budget(500), "{took:?}");
    assert_eq!(stop_reason(&events, &turn), Some(StopReason::Cancelled));
    let run = last_run(&a).await;
    assert_eq!(run.run.state, RunState::Cancelled, "{run:#?}");
    let agents = a.h.core.agents_list(a.sid.clone()).await.unwrap();
    assert!(
        agents
            .iter()
            .all(|s| s.status != app_core::dto::AgentStatus::Working)
    );
    // Kolejna wiadomość działa normalnie (kill-switch nie blokuje sesji na stałe).
    assert!(
        a.h.core
            .agents_steer(a.sid.clone(), "dalej".into())
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn steering_reaches_the_run_and_stop_cancels_it() {
    let mut a = agents().await;
    a.exec.on_command(
        "Get-ChildItem",
        FakeRun::ok("raport.pdf\nnotatki.txt\n")
            .with_effect(|_| std::thread::sleep(Duration::from_millis(300))),
    );
    a.exec.on_command("Start-Sleep", FakeRun::hanging());
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "c1",
        "shell_run",
        &json!({ "command": "Get-ChildItem" }),
    ));
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Pominęłam pliki PDF."]));
    let sent =
        a.h.core
            .turns_send(a.sid.clone(), to_delta("Delta, wypisz pliki."))
            .await
            .unwrap();
    let turn = sent.assistant_turn_id.unwrap();
    until(&mut a.h.rx, |e| {
        matches!(e, AlfaEvent::AgentStep { step, .. }
            if step.tool.as_deref() == Some("shell_run") && step.status == ReplayStatus::Running)
    })
    .await;
    a.h.core
        .agents_steer(a.sid.clone(), "Pomiń pliki PDF".into())
        .await
        .unwrap();
    let empty = a.h.core.agents_steer(a.sid.clone(), "  ".into()).await;
    assert_eq!(empty.unwrap_err().code, ErrorCode::InvalidInput);
    until(&mut a.h.rx, ends(&turn)).await;
    let last = a.h.provider.requests().last().cloned().unwrap();
    let text = serde_json::to_string(&last.messages).unwrap();
    assert!(text.contains("Pomiń pliki PDF"), "{text}");
    let run = last_run(&a).await;
    assert!(
        run.steps
            .iter()
            .any(|s| s.kind == app_core::dto::ReplayKind::Steer && s.input == "Pomiń pliki PDF"),
        "{run:#?}"
    );

    // Stop (Esc) w trakcie wiszącego polecenia: przebieg i polecenie anulowane.
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "c2",
        "shell_run",
        &json!({ "command": "Start-Sleep -Seconds 600" }),
    ));
    let (_, events) = {
        let sent =
            a.h.core
                .turns_send(a.sid.clone(), to_delta("Delta, poczekaj."))
                .await
                .unwrap();
        let turn = sent.assistant_turn_id.unwrap();
        until(&mut a.h.rx, |e| {
            matches!(e, AlfaEvent::AgentStep { step, .. }
                if step.input.contains("Start-Sleep") && step.status == ReplayStatus::Running)
        })
        .await;
        a.h.core.turns_stop(a.sid.clone()).await.unwrap();
        (turn.clone(), until(&mut a.h.rx, ends(&turn)).await)
    };
    assert!(events.iter().any(|e| matches!(
        e,
        AlfaEvent::Stop {
            reason: StopReason::Cancelled,
            ..
        }
    )));
    assert_eq!(last_run(&a).await.run.state, RunState::Cancelled);
}
