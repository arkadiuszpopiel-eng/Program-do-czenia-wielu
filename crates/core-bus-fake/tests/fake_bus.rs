//! Testy atrapy: kontrakt, determinizm, replay, wirtualny zegar.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_bus_contract::{contract_tests, Event, EventBus, EventFilter, EventKind, Level};
use core_bus_fake::{FakeBus, FakeBusError, VirtualClock};
use futures_util::StreamExt;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(FakeBus::default).await;
}

#[tokio::test]
async fn ids_and_timestamps_are_deterministic() {
    let run = || async {
        let bus = FakeBus::default();
        for i in 0..3u64 {
            bus.publish(Event::new(EventKind::Tool, Level::Info, i.into()))
                .await
                .unwrap();
            bus.clock().advance(chrono::Duration::milliseconds(10));
        }
        bus.recorded()
            .iter()
            .map(|e| (e.id, e.ts))
            .collect::<Vec<_>>()
    };
    let first = run().await;
    let second = run().await;
    assert_eq!(first, second);
    assert_eq!(first[0].0, uuid::Uuid::from_u128(1));
    assert_eq!((first[2].1 - first[0].1).num_milliseconds(), 20);
}

#[tokio::test]
async fn replay_in_custom_order() {
    let bus = FakeBus::default();
    for i in 0..3u64 {
        bus.publish(Event::new(EventKind::Ui, Level::Info, i.into()))
            .await
            .unwrap();
    }
    let mut sub = bus.subscribe(EventFilter::all()).await.unwrap();
    assert_eq!(bus.replay([2, 0, 1]).unwrap(), 3);
    let mut got = Vec::new();
    for _ in 0..3 {
        got.push(
            sub.next()
                .await
                .unwrap()
                .event()
                .unwrap()
                .payload
                .as_u64()
                .unwrap(),
        );
    }
    assert_eq!(got, vec![2, 0, 1]);
    assert_eq!(
        bus.replay([7]).unwrap_err(),
        FakeBusError::IndexOutOfRange { index: 7, len: 3 }
    );
    assert_eq!(bus.replay_all(), 3);
}

#[tokio::test]
async fn recording_survives_without_subscribers_and_can_be_cleared() {
    let bus = FakeBus::default();
    bus.publish(Event::new(
        EventKind::Voice,
        Level::Debug,
        serde_json::Value::Null,
    ))
    .await
    .unwrap();
    bus.publish(Event::new(
        EventKind::Tool,
        Level::Debug,
        serde_json::Value::Null,
    ))
    .await
    .unwrap();
    assert_eq!(bus.recorded_of_kind(&EventKind::Voice).len(), 1);
    assert_eq!(bus.stats().published, 2);
    bus.clear();
    assert!(bus.recorded().is_empty());
}

#[tokio::test]
async fn without_stamping_keeps_original_fields() {
    let clock = VirtualClock::default();
    let bus = FakeBus::new(clock).without_stamping();
    let ev = Event::new(EventKind::Gui, Level::Info, serde_json::Value::Null);
    bus.publish(ev.clone()).await.unwrap();
    assert_eq!(*bus.recorded()[0], ev);
}

#[tokio::test]
async fn dropped_subscriber_is_pruned() {
    let bus = FakeBus::default();
    let sub = bus.subscribe(EventFilter::all()).await.unwrap();
    assert_eq!(bus.stats().subscribers, 1);
    drop(sub);
    bus.publish(Event::new(
        EventKind::Gui,
        Level::Info,
        serde_json::Value::Null,
    ))
    .await
    .unwrap();
    assert_eq!(bus.stats().subscribers, 0);
}
