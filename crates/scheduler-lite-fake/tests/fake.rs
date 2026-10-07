//! Testy atrapy: kontrakt współdzielony, wirtualny zegar, nagrane zdarzenia, wstrzykiwanie błędów.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use async_trait::async_trait;
use personas_contract::PersonaId;
use scheduler_lite_contract::contract_tests::{self, Harness, poll_once};
use scheduler_lite_contract::{
    EVENT_GRANTED, EVENT_PREEMPTED, EVENT_QUEUED, EVENT_RELEASED, EVENT_TIMEOUT, Holder,
    LeaseRequest, LeaseSignal, Priority, Resource, SchedError, SchedulerLite,
};
use scheduler_lite_fake::FakeScheduler;

struct FakeHarness(FakeScheduler);

#[async_trait]
impl Harness for FakeHarness {
    type S = FakeScheduler;
    fn scheduler(&self) -> &FakeScheduler {
        &self.0
    }
    async fn advance(&self, ms: u64) {
        self.0.advance(ms);
    }
}

fn alfa() -> Holder {
    Holder::Persona(PersonaId::alfa())
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|| async { FakeHarness(FakeScheduler::new()) }).await;
}

/// ACC-F2-scheduler-lite-02: wywłaszczenie przez mowę użytkownika w 0 ms wirtualnego czasu.
#[tokio::test]
async fn preemption_decision_in_zero_virtual_ms() {
    let s = FakeScheduler::new();
    s.advance(1234);
    let narration = s
        .acquire(LeaseRequest::new(
            Resource::Speaker,
            alfa(),
            Priority::Narration,
            Duration::from_secs(1),
        ))
        .await
        .unwrap();
    let before = s.clock().now_ms();
    let mut user = Box::pin(s.acquire(LeaseRequest::new(
        Resource::Speaker,
        Holder::User,
        Priority::UserSpeech,
        Duration::from_secs(5),
    )));
    assert!(poll_once(&mut user).await.is_none());
    assert!(narration.preempt_requested());
    assert_eq!(s.clock().now_ms() - before, 0);
    let preempted = s
        .events()
        .into_iter()
        .find(|e| e.kind.to_string() == EVENT_PREEMPTED)
        .unwrap();
    assert_eq!(preempted.payload["reason"], "user_speaks");
    assert_eq!(preempted.agent.as_ref().map(|a| a.as_str()), Some("alfa"));
    drop(narration);
    assert_eq!(
        poll_once(&mut user).await.unwrap().unwrap().signal(),
        LeaseSignal::Active
    );
}

#[tokio::test]
async fn records_events_requests_and_injects_failure() {
    let s = FakeScheduler::new();
    let a = s
        .acquire(LeaseRequest::new(
            Resource::Mic,
            alfa(),
            Priority::Normal,
            Duration::from_millis(50),
        ))
        .await
        .unwrap();
    let mut b = Box::pin(s.acquire(LeaseRequest::new(
        Resource::Mic,
        Holder::User,
        Priority::Normal,
        Duration::from_millis(50),
    )));
    assert!(poll_once(&mut b).await.is_none());
    s.advance(49);
    assert!(
        poll_once(&mut b).await.is_none(),
        "przed terminem nadal czeka"
    );
    s.advance(1);
    assert!(matches!(
        poll_once(&mut b).await,
        Some(Err(SchedError::Timeout { waited_ms: 50, .. }))
    ));
    drop(a);
    let kinds: Vec<String> = s.events().iter().map(|e| e.kind.to_string()).collect();
    assert_eq!(
        kinds,
        [EVENT_GRANTED, EVENT_QUEUED, EVENT_TIMEOUT, EVENT_RELEASED]
    );
    assert_eq!(s.requests().len(), 2);
    s.fail_next(SchedError::NotStarted);
    let err = s
        .acquire(LeaseRequest::new(
            Resource::Mic,
            alfa(),
            Priority::Normal,
            Duration::ZERO,
        ))
        .await;
    assert_eq!(err.map(|_| ()), Err(SchedError::NotStarted));
    assert!(s.snapshot().is_idle());
}
