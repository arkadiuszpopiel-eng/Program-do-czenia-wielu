//! Przypadki podstawowe: przyznanie, kolejka priorytetowa, timeouty.

use crate::contract_tests::{Harness, ok, persona, poll_once, req};
use crate::{Holder, LeaseSignal, OnTimeout, Priority, Resource, SchedError, SchedulerLite};

/// Przyznanie wolnego zasobu i zwolnienie przy drop.
pub async fn grant_and_release_on_drop<H: Harness>(h: &H) {
    let s = h.scheduler();
    let lease = s
        .acquire(req(
            Resource::Speaker,
            persona("alfa"),
            Priority::Normal,
            100,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(lease.signal(), LeaseSignal::Active);
    assert_eq!(
        s.holder(&Resource::Speaker).map(|l| l.holder),
        Some(persona("alfa"))
    );
    drop(lease);
    assert_eq!(s.holder(&Resource::Speaker), None);
}

/// Wyłączność + kolejka: priorytet malejąco, w obrębie priorytetu FIFO.
pub async fn priority_queue_is_fifo_within_priority<H: Harness>(h: &H) {
    let s = h.scheduler();
    let a = s
        .acquire(req(
            Resource::Speaker,
            persona("alfa"),
            Priority::Normal,
            1000,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let mut b = Box::pin(s.acquire(req(
        Resource::Speaker,
        persona("beta"),
        Priority::Normal,
        1000,
    )));
    let mut c = Box::pin(s.acquire(req(
        Resource::Speaker,
        persona("gama"),
        Priority::Normal,
        1000,
    )));
    let mut d = Box::pin(s.acquire(req(
        Resource::Speaker,
        persona("delta"),
        Priority::Interactive,
        1000,
    )));
    assert!(poll_once(&mut b).await.is_none());
    assert!(poll_once(&mut c).await.is_none());
    assert!(poll_once(&mut d).await.is_none());
    let order: Vec<Holder> = s
        .queue(&Resource::Speaker)
        .into_iter()
        .map(|q| q.holder)
        .collect();
    assert_eq!(order, [persona("delta"), persona("beta"), persona("gama")]);
    drop(a);
    let d = ok(poll_once(&mut d).await);
    assert!(
        poll_once(&mut b).await.is_none(),
        "wyłączność: tylko jedna naraz"
    );
    drop(d);
    let b = ok(poll_once(&mut b).await);
    drop(b);
    drop(ok(poll_once(&mut c).await));
    assert_eq!(s.holder(&Resource::Speaker), None);
}

/// Timeout z polityką: `fail` dla głośnika, `ask_user` dla ekranu; `max_wait = 0` = próba.
pub async fn timeouts_follow_policy<H: Harness>(h: &H) {
    let s = h.scheduler();
    let _screen = s
        .acquire(req(
            Resource::ScreenInput,
            persona("delta"),
            Priority::Normal,
            10,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let _speaker = s
        .acquire(req(
            Resource::Speaker,
            persona("alfa"),
            Priority::Normal,
            10,
        ))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let mut screen = Box::pin(s.acquire(req(
        Resource::ScreenInput,
        persona("beta"),
        Priority::Normal,
        100,
    )));
    let mut speak = Box::pin(s.acquire(req(
        Resource::Speaker,
        persona("beta"),
        Priority::Normal,
        100,
    )));
    assert!(poll_once(&mut screen).await.is_none() && poll_once(&mut speak).await.is_none());
    h.advance(150).await;
    assert!(matches!(
        poll_once(&mut screen).await,
        Some(Err(SchedError::Timeout {
            on_timeout: OnTimeout::AskUser,
            ..
        }))
    ));
    assert!(matches!(
        poll_once(&mut speak).await,
        Some(Err(SchedError::Timeout {
            on_timeout: OnTimeout::Fail,
            ..
        }))
    ));
    let try_now = s
        .acquire(req(Resource::Speaker, persona("gama"), Priority::Normal, 0))
        .await;
    assert!(matches!(
        try_now,
        Err(SchedError::Timeout { waited_ms: 0, .. })
    ));
    let too_long = s
        .acquire(req(
            Resource::Mic,
            persona("gama"),
            Priority::Normal,
            3_600_000,
        ))
        .await;
    assert!(matches!(too_long, Err(SchedError::InvalidMaxWait { .. })));
    assert!(s.queue(&Resource::Speaker).is_empty());
}
