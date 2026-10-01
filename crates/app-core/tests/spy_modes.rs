//! Test szpiegowski trybów (ACCEPTANCE F1/F3): trzy sesje równolegle — agentka z narzędziami
//! (zapis pliku w katalogu roboczym), rozmowa głosowa (aktywna sesja) i zwykły czat — każda ze
//! swoim sekretem; 0 przecieków w żądaniach, historii, Replay, zdarzeniach, katalogach roboczych
//! i plikach danych Alfy na dysku.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use app_core::dto::{AlfaEvent, SendOptions, SessionTemplate, SettingValue, WorkdirChoice};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::voice::FakeVoice;
use common::*;
use platform_fake::FakeExec;
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;

fn tokens_in(text: &str, tokens: &[String]) -> Vec<usize> {
    (0..tokens.len())
        .filter(|i| text.contains(tokens[*i].as_str()))
        .collect()
}

/// Wszystkie pliki pod katalogiem (rekurencyjnie).
fn files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(files(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn agent_voice_and_chat_sessions_never_leak() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let voice = FakeVoice::new();
    let mut opts = options(Some(provider.clone()), Arc::new(HeadlessShell::default()));
    opts.exec = Some(Arc::new(FakeExec::new()));
    opts.voice_engine = Some(Arc::new(voice.clone()));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let events: Arc<Mutex<Vec<AlfaEvent>>> = Arc::default();
    let mut rx = core.subscribe_events();
    let sink = events.clone();
    let collector = tokio::spawn(async move {
        while let Ok(batch) = rx.recv().await {
            sink.lock().unwrap().extend(batch.iter().cloned());
        }
    });
    core.settings_set(
        "agents.verify_before_done".into(),
        SettingValue::Bool(false),
    )
    .await
    .unwrap();
    let tokens: Vec<String> = (0..3)
        .map(|i| format!("SEKRET{i}X{}", uuid::Uuid::new_v4().simple()))
        .collect();
    let mut ids = Vec::new();
    let mut workdirs = Vec::new();
    for _ in 0..3 {
        let sid = core
            .sessions_create(SessionTemplate::Empty)
            .await
            .unwrap()
            .id;
        let wd = core
            .sessions_choose_workdir(sid.clone(), WorkdirChoice::Default)
            .await
            .unwrap();
        workdirs.push(std::path::PathBuf::from(wd.path.unwrap()));
        ids.push(sid);
    }
    let (agent, spoken, chat) = (&ids[0], &ids[1], &ids[2]);
    // Agentka: zapis sekretu do pliku w katalogu roboczym swojej sesji.
    provider.push_for(
        agent,
        Script::tool_call(
            FAKE_MODEL,
            "c1",
            "fs_write",
            &json!({ "path": "kod.txt", "content": tokens[0] }),
        ),
    );
    provider.push_for(agent, Script::text(FAKE_MODEL, &["Zapisałam kod."]));
    // Głos: wypowiedź z sekretem trafia do aktywnej sesji.
    voice.mic(6_000, &[(300, 1_200)]);
    voice.stt.script(format!("Zapamiętaj kod {}", tokens[1]));
    core.app_set_active_session(Some(spoken.clone()))
        .await
        .unwrap();
    core.voice_set_mic_enabled(true).await.unwrap();
    let send = |text: String, to: Option<&str>| SendOptions {
        parent_id: None,
        text,
        addressed_to: to.map(str::to_owned),
        profile: None,
    };
    let (a, c) = tokio::join!(
        core.turns_send(
            agent.clone(),
            send(
                format!("Delta, zapisz kod {} do pliku", tokens[0]),
                Some("delta")
            )
        ),
        core.turns_send(
            chat.clone(),
            send(format!("Zapamiętaj kod {}", tokens[2]), None)
        ),
    );
    a.unwrap();
    c.unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let mut done = true;
        for sid in &ids {
            let snap = core.turns_list(sid.clone()).await.unwrap();
            done &= snap.turns.len() == 2
                && snap.turns.iter().all(|t| {
                    !matches!(
                        t.status,
                        app_core::dto::TurnStatus::Streaming | app_core::dto::TurnStatus::Queued
                    )
                });
        }
        if done || Instant::now() > deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    core.voice_set_mic_enabled(false).await.unwrap();

    // 1. Żądania: każde ze swoim sekretem i tylko z nim.
    let requests = provider.requests();
    assert!(requests.len() >= 4, "{} żądań", requests.len());
    for request in &requests {
        let session = request.meta.session.as_deref().unwrap();
        let i = ids.iter().position(|s| s == session).unwrap();
        let seen = tokens_in(&format!("{:?}", request.messages), &tokens);
        assert_eq!(seen, vec![i], "sekrety w żądaniu sesji {i}");
    }
    // 2. Historia i Replay: tylko własny sekret; przebieg agentki tylko w jej sesji.
    for (i, sid) in ids.iter().enumerate() {
        let snap = core.turns_list(sid.clone()).await.unwrap();
        assert_eq!(snap.turns.len(), 2, "sesja {i}");
        assert_eq!(tokens_in(&snap.turns[0].text, &tokens), vec![i]);
        for turn in &snap.turns {
            assert!(tokens_in(&turn.text, &tokens).iter().all(|t| *t == i));
        }
        let runs = core.agents_runs(sid.clone()).await.unwrap();
        assert_eq!(runs.len(), usize::from(i == 0), "przebiegi sesji {i}");
        let replay = serde_json::to_string(&runs).unwrap();
        assert!(tokens_in(&replay, &tokens).iter().all(|t| *t == i));
    }
    // 3. Katalogi robocze: plik agentki tylko w jej katalogu.
    assert_eq!(
        std::fs::read_to_string(workdirs[0].join("kod.txt")).unwrap(),
        tokens[0]
    );
    for wd in &workdirs[1..] {
        assert!(files(wd).is_empty(), "{}", wd.display());
    }
    // 4. Zdarzenia z sesją: treść tylko tej sesji.
    collector.abort();
    for event in events.lock().unwrap().iter() {
        let json = serde_json::to_value(event).unwrap();
        let Some(session) = json.get("session_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(i) = ids.iter().position(|s| s == session) else {
            continue;
        };
        let leaks = tokens_in(&json.to_string(), &tokens);
        assert!(
            leaks.iter().all(|t| *t == i),
            "przeciek w zdarzeniu {}",
            json["type"]
        );
    }
    // 5. Dane Alfy na dysku (bazy sesji, audyt Brokera, dziennik cofania, konfiguracja): żadnego
    //    sekretu jawnym tekstem.
    for root in [&core.paths().local, &core.paths().config] {
        for path in files(root) {
            let bytes = std::fs::read(&path).unwrap();
            let text = String::from_utf8_lossy(&bytes);
            assert!(
                tokens_in(&text, &tokens).is_empty(),
                "sekret jawnym tekstem: {}",
                path.display()
            );
        }
    }
}
