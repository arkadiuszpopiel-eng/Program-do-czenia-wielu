//! Zgodność tras abonamentowych (PLAN §1.3, ACCEPTANCE F4-05, F5-04): most nie startuje
//! z wyzwalacza/Ulepszacza ani bez zgody na harmonogram (0/100), wyłączona trasa = 0 procesów
//! (0/100), nieprzypięta wersja / inny hash = trasa wyłączona, sesja prywatna blokuje trasy
//! o nieznanej prywatności, 20/20 próśb trafia do `ApprovalSink` i do hosta MCP.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use agent_backends_contract::contract_tests::scenario::Scenario;
use agent_backends_contract::contract_tests::{RecordingSink, collect};
use agent_backends_contract::{
    AgentBackend, AgentEvent, BackendError, BridgeKind, CliPin, LaunchOrigin, LaunchRefusal,
    ScheduleConsent, TaskSpec,
};
use compliance_contract::{ChangeOrigin, Compliance, RouteId, SessionTag};
use core_bus_contract::SessionId;

fn spec(bridge: BridgeKind, source: &std::path::Path, origin: LaunchOrigin) -> TaskSpec {
    let mut s = TaskSpec::user_request(bridge, Scenario::Ok.prompt(), source, SessionId::new("z"));
    s.origin = origin;
    s
}

#[tokio::test(flavor = "multi_thread")]
async fn bridges_never_start_from_trigger_or_improver_0_of_100() {
    let h = common::harness(RecordingSink::with_pattern(vec![]), |c| {
        // Nawet jawna zgoda na harmonogram nie otwiera wyzwalaczy ani Ulepszacza.
        c.launch.scheduled.insert(
            "claude-code-cli".into(),
            ScheduleConsent { max_per_day: 1000 },
        );
        c.launch
            .scheduled
            .insert("codex-cli".into(), ScheduleConsent { max_per_day: 1000 });
    })
    .await;
    let mut refused = 0;
    for i in 0..100 {
        let bridge = BridgeKind::ALL[i % 2];
        let origin = if i % 2 == 0 {
            LaunchOrigin::Trigger {
                trigger_id: format!("t{i}"),
            }
        } else {
            LaunchOrigin::Improver
        };
        let expected = if i % 2 == 0 {
            LaunchRefusal::Trigger
        } else {
            LaunchRefusal::Improver
        };
        let r = h.backend.submit_task(spec(bridge, &h.source, origin)).await;
        assert_eq!(r, Err(BackendError::LaunchRefused { refusal: expected }));
        refused += 1;
    }
    assert_eq!(refused, 100);
    assert_eq!(
        h.backend.processes_spawned(),
        0,
        "most wystartował z wyzwalacza"
    );
    assert!(std::fs::read_dir(&h.worktrees).map_or(true, |d| d.count() == 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn schedule_requires_explicit_consent_and_daily_limit() {
    let none = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
    for i in 0..100 {
        let s = spec(
            BridgeKind::ClaudeCode,
            &none.source,
            LaunchOrigin::Scheduled {
                schedule_id: format!("s{i}"),
            },
        );
        assert_eq!(
            none.backend.submit_task(s).await,
            Err(BackendError::LaunchRefused {
                refusal: LaunchRefusal::ScheduleWithoutConsent
            })
        );
    }
    assert_eq!(none.backend.processes_spawned(), 0);

    let h = common::harness(RecordingSink::with_pattern(vec![]), |c| {
        c.launch
            .scheduled
            .insert("claude-code-cli".into(), ScheduleConsent { max_per_day: 1 });
    })
    .await;
    let first = h
        .backend
        .submit_task(spec(
            BridgeKind::ClaudeCode,
            &h.source,
            LaunchOrigin::Scheduled {
                schedule_id: "s".into(),
            },
        ))
        .await
        .unwrap();
    collect(&h.backend, &first.task, std::time::Duration::from_secs(60)).await;
    let second = h
        .backend
        .submit_task(spec(
            BridgeKind::ClaudeCode,
            &h.source,
            LaunchOrigin::Scheduled {
                schedule_id: "s".into(),
            },
        ))
        .await;
    assert_eq!(
        second,
        Err(BackendError::LaunchRefused {
            refusal: LaunchRefusal::ScheduleDailyLimit { limit: 1 }
        })
    );
    let codex = h
        .backend
        .submit_task(spec(
            BridgeKind::Codex,
            &h.source,
            LaunchOrigin::Scheduled {
                schedule_id: "s".into(),
            },
        ))
        .await;
    assert!(
        matches!(codex, Err(BackendError::LaunchRefused { .. })),
        "zgoda jest per trasa"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn disabled_route_means_zero_calls_0_of_100() {
    let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
    for (route, bridge) in [
        ("claude-code-cli", BridgeKind::ClaudeCode),
        ("codex-cli", BridgeKind::Codex),
    ] {
        let id = RouteId::new(route).unwrap();
        h.compliance
            .set_enabled(&id, false, ChangeOrigin::User)
            .await
            .unwrap();
        for _ in 0..50 {
            let r = h
                .backend
                .submit_task(spec(bridge, &h.source, LaunchOrigin::UserRequest))
                .await;
            assert!(
                matches!(r, Err(BackendError::RouteNotAllowed { .. })),
                "{r:?}"
            );
        }
    }
    assert_eq!(
        h.backend.processes_spawned(),
        0,
        "wywołanie wyłączonej trasy"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn private_session_blocks_route_with_unknown_privacy() {
    let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
    let mut s = spec(BridgeKind::ClaudeCode, &h.source, LaunchOrigin::UserRequest);
    s.privacy = SessionTag::Private;
    assert!(matches!(
        h.backend.submit_task(s).await,
        Err(BackendError::RouteNotAllowed { .. })
    ));
    assert_eq!(h.backend.processes_spawned(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn unpinned_version_or_changed_binary_disables_route() {
    let h = common::harness(RecordingSink::with_pattern(vec![]), |c| {
        for b in c.bridges.values_mut() {
            b.pin = CliPin {
                versions: vec!["2.1.0".into()],
                sha256: None,
            };
        }
    })
    .await;
    for bridge in BridgeKind::ALL {
        for _ in 0..3 {
            let r = h
                .backend
                .submit_task(spec(bridge, &h.source, LaunchOrigin::UserRequest))
                .await;
            assert_eq!(
                r,
                Err(BackendError::VersionNotPinned {
                    program: bridge.program().into(),
                    found: Some(common::FAKE_VERSION.into())
                })
            );
        }
    }
    assert_eq!(
        h.backend.processes_spawned(),
        1,
        "tylko jedno `--version` (pamięć podręczna), zero zadań"
    );

    let hashed = common::harness(RecordingSink::with_pattern(vec![]), |c| {
        for b in c.bridges.values_mut() {
            b.pin.sha256 = Some("0".repeat(64));
        }
    })
    .await;
    let r = hashed
        .backend
        .submit_task(spec(
            BridgeKind::ClaudeCode,
            &hashed.source,
            LaunchOrigin::UserRequest,
        ))
        .await;
    assert_eq!(
        r,
        Err(BackendError::BinaryHashMismatch {
            program: "claude".into()
        })
    );

    let missing = common::harness(RecordingSink::with_pattern(vec![]), |c| {
        for b in c.bridges.values_mut() {
            b.program = "/nie/ma/takiego/cli".into();
        }
    })
    .await;
    let r = missing
        .backend
        .submit_task(spec(
            BridgeKind::Codex,
            &missing.source,
            LaunchOrigin::UserRequest,
        ))
        .await;
    assert!(matches!(r, Err(BackendError::CliNotFound { .. })));
    assert_eq!(missing.backend.processes_spawned(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn twenty_of_twenty_permission_requests_reach_sink_and_mcp_host() {
    let sink = RecordingSink::with_pattern(vec![true, false, true]);
    let h = common::harness(sink.clone(), |_| {}).await;
    let mut s = spec(BridgeKind::ClaudeCode, &h.source, LaunchOrigin::UserRequest);
    s.prompt = Scenario::Permission(20).prompt();
    let handle = h.backend.submit_task(s).await.unwrap();
    let events = collect(&h.backend, &handle.task, std::time::Duration::from_secs(60)).await;
    assert_eq!(
        h.host.approvals().len(),
        20,
        "prośby nie przeszły przez narzędzie MCP `approve`"
    );
    let mine: Vec<_> = sink
        .seen()
        .into_iter()
        .filter(|r| r.task == handle.task)
        .collect();
    assert_eq!(mine.len(), 20);
    assert!(
        mine.iter()
            .all(|r| r.tool == "Bash" && r.bridge == BridgeKind::ClaudeCode)
    );
    assert!(matches!(
        &events.last().unwrap().event,
        AgentEvent::Done { .. }
    ));
    assert!(events.iter().all(|e| e.unverified_by_alfa));
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_specs_are_rejected_before_any_process() {
    let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
    let mut empty = spec(BridgeKind::ClaudeCode, &h.source, LaunchOrigin::UserRequest);
    empty.prompt = "  ".into();
    let mut tools = spec(BridgeKind::ClaudeCode, &h.source, LaunchOrigin::UserRequest);
    tools.allowed_tools = vec!["Bash,Write".into()];
    let mut model = spec(BridgeKind::ClaudeCode, &h.source, LaunchOrigin::UserRequest);
    model.model = Some(["--dangerously", "-skip-permissions"].concat());
    for s in [empty, tools, model] {
        assert!(matches!(
            h.backend.submit_task(s).await,
            Err(BackendError::InvalidSpec(_))
        ));
    }
    assert_eq!(h.backend.processes_spawned(), 0);
}

/// Każde zdarzenie mostu trafia na magistralę z oznaczeniem „niezależnie niezweryfikowane”.
#[tokio::test(flavor = "multi_thread")]
async fn bus_events_are_marked_unverified() {
    let clock = core_bus_fake::VirtualClock::starting_at(chrono::Utc::now());
    let bus = std::sync::Arc::new(core_bus_fake::FakeBus::new(clock));
    let h = common::harness(RecordingSink::with_pattern(vec![]), |_| {}).await;
    let backend = h.backend.with_bus(bus.clone());
    let handle = backend
        .submit_task(spec(
            BridgeKind::Codex,
            &h.source,
            LaunchOrigin::UserRequest,
        ))
        .await
        .unwrap();
    let events = collect(&backend, &handle.task, std::time::Duration::from_secs(60)).await;
    let kind =
        core_bus_contract::EventKind::Custom(agent_backends_contract::EVENT_TASK_EVENT.into());
    let mut published = Vec::new();
    for _ in 0..100 {
        published = bus.recorded_of_kind(&kind);
        if published.len() == events.len() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(published.len(), events.len());
    assert!(
        published
            .iter()
            .all(|e| e.payload["unverified_by_alfa"] == true)
    );
}
