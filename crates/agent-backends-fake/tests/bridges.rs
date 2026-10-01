//! Mosty od końca do końca z fałszywym CLI: worktree, postęp ≤ 1 s, anulowanie drzewa procesów,
//! odporność parsera (śmieci, długie linie, crash, błąd), środowisko bez sekretów, wznawianie,
//! limit czasu zatwierdzeń.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agent_backends_contract::contract_tests::scenario::Scenario;
use agent_backends_contract::contract_tests::{RecordingSink, assert_grammar, budget, collect};
use agent_backends_contract::policy::is_secret_env_name;
use agent_backends_contract::{
    AgentBackend, AgentEvent, BackendError, BridgeKind, TaskSpec, WorkdirMode,
};
use core_bus_contract::SessionId;

fn spec(bridge: BridgeKind, scenario: &Scenario, source: &std::path::Path) -> TaskSpec {
    TaskSpec::user_request(bridge, scenario.prompt(), source, SessionId::new("test"))
}

fn outputs(events: &[agent_backends_contract::AgentEventEnvelope]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match &e.event {
            AgentEvent::Output {
                text,
                partial: false,
                ..
            } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

const LIMIT: Duration = Duration::from_secs(60);

#[tokio::test(flavor = "multi_thread")]
async fn ok_flow_runs_in_worktree_not_in_user_dir() {
    for bridge in BridgeKind::ALL {
        let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
        let handle = h
            .backend
            .submit_task(spec(bridge, &Scenario::Ok, &h.source))
            .await
            .unwrap();
        let events = collect(&h.backend, &handle.task, LIMIT).await;
        assert_grammar(&events);
        assert!(
            handle
                .workdir
                .starts_with(h.worktrees.canonicalize().unwrap())
        );
        assert!(
            handle.workdir.join("wynik.txt").exists(),
            "{bridge}: plik nie powstał w worktree"
        );
        assert!(
            handle.workdir.join("README.txt").exists(),
            "{bridge}: worktree bez plików repo"
        );
        assert!(
            !h.source.join("wynik.txt").exists(),
            "{bridge}: CLI pisało w katalogu użytkownika"
        );
        let kinds: Vec<&str> = events
            .iter()
            .map(|e| match &e.event {
                AgentEvent::Started { .. } => "started",
                AgentEvent::ColdStart { .. } => "cold",
                AgentEvent::SessionStarted { .. } => "session",
                AgentEvent::Plan { .. } => "plan",
                AgentEvent::FileChanged { .. } => "file",
                AgentEvent::ToolRequest { .. } => "tool",
                AgentEvent::ToolFinished { .. } => "tool_done",
                AgentEvent::Usage { .. } => "usage",
                AgentEvent::Output { partial: true, .. } => "partial",
                AgentEvent::Done { .. } => "done",
                _ => "other",
            })
            .collect();
        for k in [
            "started",
            "cold",
            "session",
            "plan",
            "file",
            "tool",
            "tool_done",
            "usage",
            "partial",
            "done",
        ] {
            assert!(kinds.contains(&k), "{bridge}: brak `{k}` w {kinds:?}");
        }
        let cold = events.iter().find_map(|e| match e.event {
            AgentEvent::ColdStart { ms } => Some(ms),
            _ => None,
        });
        eprintln!("{bridge}: zimny start {cold:?} ms");
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

#[tokio::test(flavor = "multi_thread")]
async fn progress_latency_within_budget() {
    let limit = budget(Duration::from_secs(1), "opóźnienie postępu mostu");
    for bridge in BridgeKind::ALL {
        let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
        let scenario = Scenario::Slow {
            n: 5,
            interval_ms: 300,
        };
        let handle = h
            .backend
            .submit_task(spec(bridge, &scenario, &h.source))
            .await
            .unwrap();
        let mut stream = h.backend.events(&handle.task).unwrap();
        let mut worst = 0u128;
        let mut seen = 0;
        while let Ok(Some(ev)) =
            tokio::time::timeout(LIMIT, futures_util::StreamExt::next(&mut stream)).await
        {
            if let AgentEvent::Output {
                text,
                partial: false,
                ..
            } = &ev.event
                && let Some(t) = text.strip_prefix("t=").and_then(|t| t.parse::<u128>().ok())
            {
                worst = worst.max(now_ms().saturating_sub(t));
                seen += 1;
            }
            if ev.event.is_terminal() {
                break;
            }
        }
        eprintln!("{bridge}: najgorsze opóźnienie postępu {worst} ms ({seen} zdarzeń)");
        assert_eq!(seen, 5);
        assert!(worst <= limit.as_millis(), "{bridge}: {worst} ms");
    }
}

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .is_ok_and(|s| s.split_whitespace().nth(2).is_none_or(|state| state != "Z"))
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_kills_process_tree() {
    for bridge in BridgeKind::ALL {
        let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
        let handle = h
            .backend
            .submit_task(spec(bridge, &Scenario::Hang, &h.source))
            .await
            .unwrap();
        let mut stream = h.backend.events(&handle.task).unwrap();
        let mut grandchild = 0u32;
        while let Ok(Some(ev)) =
            tokio::time::timeout(LIMIT, futures_util::StreamExt::next(&mut stream)).await
        {
            if let AgentEvent::Output { text, .. } = &ev.event
                && let Some(pid) = text.strip_prefix("child_pid=")
            {
                grandchild = pid.parse().unwrap();
                break;
            }
        }
        assert!(grandchild > 0);
        let limit = budget(Duration::from_secs(2), "anulowanie drzewa procesów");
        let started = std::time::Instant::now();
        h.backend.cancel(&handle.task).await.unwrap();
        let events = collect(&h.backend, &handle.task, limit).await;
        assert!(matches!(
            events.last().map(|e| &e.event),
            Some(AgentEvent::Error {
                error: BackendError::Cancelled
            })
        ));
        assert!(started.elapsed() <= limit);
        #[cfg(unix)]
        {
            let deadline = std::time::Instant::now() + limit;
            while alive(grandchild) && std::time::Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            assert!(
                !alive(grandchild),
                "{bridge}: proces-wnuk {grandchild} przeżył anulowanie"
            );
        }
        assert_eq!(
            h.backend.cancel(&handle.task).await,
            Err(BackendError::TaskFinished)
        );
        assert!(matches!(
            h.backend.steer(&handle.task, "x".into()).await,
            Err(BackendError::TaskFinished)
        ));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn parser_is_tolerant_and_failures_are_reported() {
    for bridge in BridgeKind::ALL {
        let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
        let run = |s: Scenario| {
            let spec = spec(bridge, &s, &h.source);
            let backend = &h.backend;
            async move {
                let handle = backend.submit_task(spec).await.unwrap();
                let events = collect(backend, &handle.task, LIMIT).await;
                assert_grammar(&events);
                events
            }
        };
        let garbage = run(Scenario::Garbage).await;
        let warnings = garbage
            .iter()
            .filter(|e| matches!(e.event, AgentEvent::Warning { .. }))
            .count();
        assert!(warnings >= 2, "{bridge}: {garbage:?}");
        assert!(
            matches!(&garbage.last().unwrap().event, AgentEvent::Done { result } if !result.is_error)
        );

        let long = run(Scenario::LongLine(3 * 1024 * 1024)).await;
        assert!(long.iter().any(
            |e| matches!(&e.event, AgentEvent::Warning { message } if message.contains("za długą"))
        ));
        assert!(outputs(&long).iter().any(|t| t == "po długiej linii"));

        let error = run(Scenario::ErrorResult).await;
        assert!(
            matches!(&error.last().unwrap().event, AgentEvent::Done { result } if result.is_error)
        );

        let crash = run(Scenario::Crash).await;
        assert!(matches!(
            &crash.last().unwrap().event,
            AgentEvent::Error {
                error: BackendError::CliExited { code: Some(3), .. }
            }
        ));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_environment_has_no_alfa_secrets() {
    for bridge in BridgeKind::ALL {
        let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
        let handle = h
            .backend
            .submit_task(spec(bridge, &Scenario::Env, &h.source))
            .await
            .unwrap();
        let events = collect(&h.backend, &handle.task, LIMIT).await;
        let names = outputs(&events).join(",");
        assert!(names.contains("PATH"), "{bridge}: {names}");
        // `cargo test` ustawia CARGO_* w procesie testu — CLI nie może ich dostać (lista dozwolona).
        assert!(std::env::var("CARGO_PKG_NAME").is_ok());
        for name in names.split(',').filter(|n| !n.is_empty()) {
            assert!(!is_secret_env_name(name), "{bridge}: CLI dostało `{name}`");
            assert!(!name.starts_with("CARGO"), "{bridge}: CLI dostało `{name}`");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn resume_reuses_session_and_worktree() {
    for bridge in BridgeKind::ALL {
        let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
        let first = h
            .backend
            .submit_task(spec(bridge, &Scenario::Ok, &h.source))
            .await
            .unwrap();
        let events = collect(&h.backend, &first.task, LIMIT).await;
        let AgentEvent::Done { result } = &events.last().unwrap().event else {
            panic!("{events:?}");
        };
        let session = result.session.clone().unwrap();
        assert_eq!(session.workdir, first.workdir);
        let again = h
            .backend
            .resume(session.clone(), spec(bridge, &Scenario::Ok, &h.source))
            .await
            .unwrap();
        assert_eq!(again.workdir, first.workdir);
        let events = collect(&h.backend, &again.task, LIMIT).await;
        assert!(events.iter().any(
            |e| matches!(&e.event, AgentEvent::SessionStarted { session: s } if s.id == session.id)
        ));
        let mut foreign = session.clone();
        foreign.workdir = h.source.clone();
        let refused = h
            .backend
            .resume(foreign, spec(bridge, &Scenario::Ok, &h.source))
            .await;
        assert!(
            matches!(refused, Err(BackendError::Workspace(_))),
            "{refused:?}"
        );
        let other = BridgeKind::ALL.into_iter().find(|b| *b != bridge).unwrap();
        let mismatch = h
            .backend
            .resume(session, spec(other, &Scenario::Ok, &h.source))
            .await;
        assert!(matches!(mismatch, Err(BackendError::InvalidSpec(_))));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn unanswered_permission_times_out_as_denial() {
    for bridge in BridgeKind::ALL {
        let h = common::harness(RecordingSink::with_pattern(vec![]), |c| {
            c.approval_timeout = Duration::from_millis(200);
        })
        .await;
        let handle = h
            .backend
            .submit_task(spec(bridge, &Scenario::Permission(1), &h.source))
            .await
            .unwrap();
        let events = collect(&h.backend, &handle.task, LIMIT).await;
        assert!(events.iter().any(|e| matches!(&e.event, AgentEvent::PermissionResolved { timed_out: true, decision, .. } if !decision.is_allow())));
        assert!(
            matches!(&events.last().unwrap().event, AgentEvent::Done { result } if result.text == "decisions=deny")
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn copy_mode_and_non_git_source() {
    let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
    let plain = common::temp_dir("plain");
    std::fs::write(plain.join("a.txt"), "a").unwrap();
    std::fs::create_dir_all(plain.join("pod")).unwrap();
    std::fs::write(plain.join("pod").join("b.txt"), "b").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc/hostname", plain.join("link")).unwrap();
    let mut s = spec(BridgeKind::ClaudeCode, &Scenario::Ok, &plain);
    s.workdir.mode = WorkdirMode::Copy;
    let handle = h.backend.submit_task(s).await.unwrap();
    collect(&h.backend, &handle.task, LIMIT).await;
    assert!(handle.workdir.join("pod").join("b.txt").exists());
    assert!(
        !handle.workdir.join("link").exists(),
        "dowiązanie symboliczne skopiowane"
    );
    assert!(!plain.join("wynik.txt").exists());
    let mut bad = spec(BridgeKind::ClaudeCode, &Scenario::Ok, &plain.join("nie-ma"));
    bad.workdir.mode = WorkdirMode::Copy;
    assert!(matches!(
        h.backend.submit_task(bad).await,
        Err(BackendError::Workspace(_))
    ));
}
