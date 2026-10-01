//! Katalog roboczy sesji (zakres narzędzi agentek) i intencja „uruchom w terminalu": wybór
//! w dialogu powłoki, odrzucenie katalogów danych Alfy, sesja bez katalogu = odpowiedź bez
//! narzędzi; polecenie z kroku `shell_terminal` trafia do terminala bez wykonania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::ErrorCode;
use app_core::dto::{IntentKind, ReplayStatus, WorkdirChoice};
use common::agents::*;
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workdir_dialog_validation_and_no_tools_without_workdir() {
    let mut a = agents().await;
    let core = a.h.core.clone();
    let picked = a.h.dir.path().join("user/Projekt");
    std::fs::create_dir_all(&picked).unwrap();
    a.h.shell.answer_dialog(Some(picked.clone()));
    let view = core
        .sessions_choose_workdir(a.sid.clone(), WorkdirChoice::Dialog)
        .await
        .unwrap();
    assert_eq!(view.path.as_deref(), Some(picked.to_str().unwrap()));
    assert!(a.h.shell.calls().iter().any(|c| c == "pick_folder"));
    // Anulowany dialog nie zmienia wyboru.
    a.h.shell.answer_dialog(None);
    let same = core
        .sessions_choose_workdir(a.sid.clone(), WorkdirChoice::Dialog)
        .await
        .unwrap();
    assert_eq!(same.path, view.path);
    // Katalogi danych Alfy (bazy sesji, konfiguracja) i nieistniejące ścieżki — odrzucone.
    for (dir, code) in [
        (a.h.dir.path().join("local"), ErrorCode::Forbidden),
        (a.h.dir.path().join("config"), ErrorCode::Forbidden),
        (a.h.dir.path().to_path_buf(), ErrorCode::Forbidden),
        (a.h.dir.path().join("brak"), ErrorCode::InvalidInput),
    ] {
        std::fs::create_dir_all(a.h.dir.path().join("config")).unwrap();
        a.h.shell.answer_dialog(Some(dir.clone()));
        let err = core
            .sessions_choose_workdir(a.sid.clone(), WorkdirChoice::Dialog)
            .await
            .unwrap_err();
        assert_eq!(err.code, code, "{}: {err:?}", dir.display());
    }
    assert_eq!(
        core.sessions_workdir(a.sid.clone()).await.unwrap().path,
        view.path
    );

    // Bez katalogu roboczego Delta odpowiada zwykłym czatem (echo atrapy), bez przebiegu.
    let none = core
        .sessions_choose_workdir(a.sid.clone(), WorkdirChoice::None)
        .await
        .unwrap();
    assert_eq!(none.path, None);
    let (turn, _) = run_turn(&mut a, "Delta, co słychać?").await;
    let snapshot = core.turns_list(a.sid.clone()).await.unwrap();
    let reply = snapshot.turns.iter().find(|t| t.id == turn).unwrap();
    assert!(reply.text.starts_with("Echo:"), "{}", reply.text);
    assert!(core.agents_runs(a.sid.clone()).await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn terminal_intent_opens_terminal_without_running_the_command() {
    let mut a = agents().await;
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "c1",
        "shell_terminal",
        &json!({ "command": "winget upgrade --all" }),
    ));
    a.h.provider.push(Script::text(
        FAKE_MODEL,
        &["Polecenie czeka w terminalu do uruchomienia."],
    ));
    let (turn, _) = run_turn(&mut a, "Delta, zaktualizuj programy.").await;
    assert!(a.exec.runs().is_empty(), "polecenie nie zostało wykonane");
    let run = last_run(&a).await;
    let step = run
        .steps
        .iter()
        .find(|s| s.tool.as_deref() == Some("shell_terminal"))
        .expect("krok shell_terminal");
    assert_eq!(step.status, ReplayStatus::NeedsConfirmation, "{step:#?}");
    let intent = step.intent.clone().expect("intencja w kroku");
    assert_eq!(intent.kind, IntentKind::OpenInTerminal);
    assert_eq!(intent.command.as_deref(), Some("winget upgrade --all"));
    let snapshot = a.h.core.turns_list(a.sid.clone()).await.unwrap();
    let reply = snapshot.turns.iter().find(|t| t.id == turn).unwrap();
    assert!(reply.tools.iter().any(|t| t.intent.is_some()));

    a.h.core
        .agents_open_terminal(step.id.clone())
        .await
        .unwrap();
    let opened = a.h.shell.calls();
    assert!(
        opened.iter().any(|c| c.starts_with("open_terminal:")),
        "{opened:?}"
    );
    assert!(
        a.exec.runs().is_empty(),
        "terminal bez automatycznego wykonania"
    );
    // Krok bez intencji terminala i krok obcej sesji — czytelne błędy.
    let other =
        a.h.core
            .sessions_create(app_core::dto::SessionTemplate::Empty)
            .await
            .unwrap()
            .id;
    let foreign = step.id.replacen(a.sid.as_str(), other.as_str(), 1);
    assert!(a.h.core.agents_open_terminal(foreign).await.is_err());
    let bad = a.h.core.agents_open_terminal("x".into()).await.unwrap_err();
    assert_eq!(bad.code, ErrorCode::InvalidInput);
}
