//! Z.8: pełna ścieżka kierowca → kontekst sesji → Router → model → Broker → dziennik.
//! Porty maszyny i model są atrapami; żaden test nie wykonuje połączenia z Internetem.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::time::Duration;

use providers_contract::ModelProvider;
use providers_fake::{FAKE_MODEL, Script};
use router_contract::RouteKind;
use safety_broker_contract::{
    ApprovalChannel, ApprovalStatus, AutonomyLevel, AutonomyTarget, Broker, ModelLocation,
    contract_tests,
};
use sessions_contract::NewTurn;
use support::*;

#[tokio::test(start_paused = true)]
async fn local_verdict_reaches_broker_audit_and_timeline() {
    let rig = Rig::new(vec![NewTurn::user("Pobierz kurs EUR"), assistant("local")]);
    let (local, provider) = router(
        "local",
        RouteKind::Local,
        Script::text(FAKE_MODEL, &[ALLOW]),
    );
    let id = rig.ask(Some(ModelLocation::Local)).await;
    let (stop, mut notes) = rig.start(Some(&local), None);
    let entry = note(&mut notes).await;
    stop.cancel();

    assert_eq!(entry.session, rig.session.as_str());
    assert!(entry.title.contains("zezwolił raz"), "{}", entry.title);
    assert!(!entry.detail.contains("nie przyjął"));
    assert!(!entry.warn);
    let ApprovalStatus::Approved { token: Some(token) } =
        rig.broker.approval_status(id, &rig.holder()).unwrap()
    else {
        panic!("werdykt powinien wydać token")
    };
    rig.broker
        .verify(
            &token,
            &rig.action(Some(ModelLocation::Local)).capability,
            &rig.holder(),
        )
        .unwrap();
    assert!(rig.broker.grants().is_empty(), "tylko zgoda jednorazowa");
    assert!(rig.broker.pending().is_empty());
    assert!(
        rig.audit
            .names()
            .iter()
            .any(|n| n == safety_broker_contract::EVENT_JUDGE_DECIDED)
    );
    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].meta.route.review);
    assert!(requests[0].tools.is_empty());
    assert_eq!(requests[0].params.temperature, Some(0.0));
    assert!(format!("{:?}", requests[0].messages).contains("Pobierz kurs EUR"));
}

#[tokio::test(start_paused = true)]
async fn cloud_judge_requires_cloud_in_both_history_and_action() {
    for history in ["local", "anthropic"] {
        for location in [None, Some(ModelLocation::Local), Some(ModelLocation::Cloud)] {
            let rig = Rig::new(vec![NewTurn::user("Pobierz kurs EUR"), assistant(history)]);
            let (hybrid, cloud) = router(
                "anthropic",
                RouteKind::Api,
                Script::text(FAKE_MODEL, &[ALLOW]),
            );
            let id = rig.ask(location).await;
            let (stop, mut notes) = rig.start(None, Some(&hybrid));
            let entry = note(&mut notes).await;
            stop.cancel();
            let permitted = history == "anthropic" && location == Some(ModelLocation::Cloud);
            assert_eq!(
                cloud.requests().len(),
                usize::from(permitted),
                "{history}, {location:?}"
            );
            assert_eq!(
                matches!(
                    rig.broker.approval_status(id, &rig.holder()).unwrap(),
                    ApprovalStatus::Approved { .. }
                ),
                permitted
            );
            if !permitted {
                assert!(rig.broker.pending().iter().any(|p| p.request.id == id));
                assert!(entry.title.contains("przekazał Tobie"));
            }
        }
    }
}

#[tokio::test(start_paused = true)]
async fn unknown_latest_origin_never_sends_history_to_cloud() {
    let rig = Rig::new(vec![
        NewTurn::user("Pobierz kurs EUR"),
        assistant("anthropic"),
        NewTurn::assistant("delta", "Przygotowuję kolejne pobranie"),
    ]);
    let (hybrid, cloud) = router(
        "anthropic",
        RouteKind::Api,
        Script::text(FAKE_MODEL, &[ALLOW]),
    );
    let id = rig.ask(Some(ModelLocation::Cloud)).await;
    let (stop, mut notes) = rig.start(None, Some(&hybrid));
    let entry = note(&mut notes).await;
    stop.cancel();
    assert!(
        cloud.requests().is_empty(),
        "starsza odpowiedź nie ustala pochodzenia najnowszej"
    );
    assert!(entry.title.contains("przekazał Tobie"));
    assert!(rig.broker.pending().iter().any(|p| p.request.id == id));
}

#[tokio::test(start_paused = true)]
async fn invalid_response_and_unavailable_model_return_to_owner() {
    for script in [
        Script::text(FAKE_MODEL, &["zgadzam się"]),
        Script::http_error(503, None),
        Script::stall(),
    ] {
        let rig = Rig::new(vec![NewTurn::user("Pobierz kurs EUR"), assistant("local")]);
        let (local, _) = router("local", RouteKind::Local, script);
        let id = rig.ask(Some(ModelLocation::Local)).await;
        let (stop, mut notes) = rig.start(Some(&local), None);
        let entry = note(&mut notes).await;
        stop.cancel();
        assert!(entry.title.contains("przekazał Tobie"), "{}", entry.title);
        assert_eq!(
            rig.broker.approval_status(id, &rig.holder()),
            Ok(ApprovalStatus::Pending)
        );
        assert!(rig.broker.pending().iter().any(|p| p.request.id == id));
    }
}

#[tokio::test(start_paused = true)]
async fn local_router_failure_never_falls_back_to_hybrid() {
    for configured in [false, true] {
        let rig = Rig::new(vec![NewTurn::user("Pobierz kurs EUR"), assistant("local")]);
        let (local, provider) = router("local", RouteKind::Local, Script::http_error(503, None));
        if !configured {
            local.unregister(provider.id());
        }
        let (hybrid, cloud) = router(
            "anthropic",
            RouteKind::Api,
            Script::text(FAKE_MODEL, &[ALLOW]),
        );
        let id = rig.ask(Some(ModelLocation::Local)).await;
        let (stop, mut notes) = rig.start(Some(&local), Some(&hybrid));
        let entry = note(&mut notes).await;
        stop.cancel();
        assert!(
            cloud.calls().is_empty(),
            "awaria lokalna nie daje zgody na chmurę"
        );
        assert!(entry.title.contains("przekazał Tobie"));
        assert!(rig.broker.pending().iter().any(|p| p.request.id == id));
    }
}

#[tokio::test(start_paused = true)]
async fn cloud_run_can_use_local_judge_when_hybrid_is_absent() {
    let rig = Rig::new(vec![
        NewTurn::user("Pobierz kurs EUR"),
        assistant("anthropic"),
    ]);
    let (local, provider) = router(
        "local",
        RouteKind::Local,
        Script::text(FAKE_MODEL, &[ALLOW]),
    );
    let id = rig.ask(Some(ModelLocation::Cloud)).await;
    let (stop, mut notes) = rig.start(Some(&local), None);
    let entry = note(&mut notes).await;
    stop.cancel();
    assert_eq!(provider.requests().len(), 1);
    assert!(entry.title.contains("zezwolił raz"));
    assert!(matches!(
        rig.broker.approval_status(id, &rig.holder()),
        Ok(ApprovalStatus::Approved { .. })
    ));
}

#[tokio::test(start_paused = true)]
async fn owner_boundary_overrides_model_allow_in_composed_path() {
    let rig = Rig::new(vec![
        NewTurn::user("Pobierz kurs EUR. Nie wysyłaj danych"),
        assistant("local"),
    ]);
    let (local, provider) = router(
        "local",
        RouteKind::Local,
        Script::text(FAKE_MODEL, &[ALLOW]),
    );
    let id = rig.ask(Some(ModelLocation::Local)).await;
    let (stop, mut notes) = rig.start(Some(&local), None);
    let entry = note(&mut notes).await;
    stop.cancel();
    assert_eq!(provider.requests().len(), 1);
    assert!(entry.title.contains("przekazał Tobie"));
    assert!(entry.detail.contains(approval_judge_contract::GUARD_NOTE));
    assert!(rig.broker.pending().iter().any(|p| p.request.id == id));
}

#[tokio::test(start_paused = true)]
async fn mode_change_during_review_rejects_stale_allow_and_logs_it() {
    let rig = Rig::new(vec![NewTurn::user("Pobierz kurs EUR"), assistant("local")]);
    let (local, provider) = router(
        "local",
        RouteKind::Local,
        Script::text(FAKE_MODEL, &[ALLOW]).delayed(Duration::from_secs(1)),
    );
    let id = rig.ask(Some(ModelLocation::Local)).await;
    let (stop, mut notes) = rig.start(Some(&local), None);
    tokio::time::timeout(Duration::from_secs(1), async {
        while provider.requests().is_empty() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("sędzia powinien rozpocząć ocenę");
    contract_tests::set_level(
        &*rig.broker,
        &rig.clock,
        AutonomyTarget::Global,
        AutonomyLevel::L0,
    )
    .await;
    let entry = note(&mut notes).await;
    stop.cancel();
    assert!(
        entry.detail.contains("nie przyjął werdyktu"),
        "{}",
        entry.detail
    );
    assert!(!matches!(
        rig.broker.approval_status(id, &rig.holder()),
        Ok(ApprovalStatus::Approved { .. })
    ));
    assert!(rig.broker.grants().is_empty());
}
