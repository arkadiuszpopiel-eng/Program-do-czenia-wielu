//! Testy `BroadcastBus`: kontraktowe, wieloproducentowe i backpressure.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::Arc;

use core_bus_contract::{
    contract_tests, BusError, BusItem, Event, EventBus, EventFilter, EventKind, Level,
};
use core_bus_impl::{BroadcastBus, BusConfig};
use futures_util::StreamExt;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(BroadcastBus::default).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn many_producers_all_events_arrive() {
    const PRODUCERS: u64 = 8;
    const PER_PRODUCER: u64 = 200;
    let bus = Arc::new(BroadcastBus::new(BusConfig {
        buffer_per_subscriber: 8192,
        ..BusConfig::default()
    }));
    let mut sub = bus.subscribe(EventFilter::prefix("mp.")).await.unwrap();

    let mut handles = Vec::new();
    for p in 0..PRODUCERS {
        let bus = Arc::clone(&bus);
        handles.push(tokio::spawn(async move {
            for i in 0..PER_PRODUCER {
                let ev = Event::new(
                    EventKind::Custom("mp.tick".into()),
                    Level::Info,
                    serde_json::json!({"p": p, "i": i}),
                );
                bus.publish(ev).await.unwrap();
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let mut seen = BTreeSet::new();
    let mut per_producer_last = vec![None::<u64>; PRODUCERS as usize];
    for _ in 0..(PRODUCERS * PER_PRODUCER) {
        let item = tokio::time::timeout(std::time::Duration::from_secs(5), sub.next())
            .await
            .unwrap()
            .unwrap();
        let ev = item.event().expect("bez lagu przy dużym buforze");
        let p = ev.payload["p"].as_u64().unwrap();
        let i = ev.payload["i"].as_u64().unwrap();
        // kolejność per wydawca zachowana
        assert!(per_producer_last[p as usize].is_none_or(|last| last < i));
        per_producer_last[p as usize] = Some(i);
        seen.insert((p, i));
    }
    assert_eq!(seen.len() as u64, PRODUCERS * PER_PRODUCER);
    assert_eq!(bus.stats().dropped, 0);
    assert_eq!(bus.stats().published, PRODUCERS * PER_PRODUCER);
}

#[tokio::test]
async fn slow_subscriber_gets_lagged_and_counter_grows() {
    let bus = BroadcastBus::new(BusConfig {
        buffer_per_subscriber: 4,
        ..BusConfig::default()
    });
    let mut sub = bus.subscribe(EventFilter::all()).await.unwrap();
    for i in 0..10u64 {
        // publish nie blokuje mimo pełnego bufora
        bus.publish(Event::new(EventKind::Ui, Level::Info, i.into()))
            .await
            .unwrap();
    }
    let first = sub.next().await.unwrap();
    match first {
        BusItem::Lagged(n) => assert_eq!(n, 6),
        BusItem::Event(_) => panic!("oczekiwano Lagged"),
    }
    assert_eq!(bus.stats().dropped, 6);
    // pozostałe 4 zdarzenia są dostępne w kolejności
    for i in 6..10u64 {
        let ev = sub.next().await.unwrap();
        assert_eq!(ev.event().unwrap().payload, serde_json::Value::from(i));
    }
}

#[tokio::test]
async fn subscriber_count_follows_stream_lifetime() {
    let bus = BroadcastBus::default();
    let a = bus.subscribe(EventFilter::all()).await.unwrap();
    let b = bus.subscribe(EventFilter::all()).await.unwrap();
    assert_eq!(bus.stats().subscribers, 2);
    drop(a);
    assert_eq!(bus.stats().subscribers, 1);
    drop(b);
    assert_eq!(bus.stats().subscribers, 0);
}

#[tokio::test]
async fn oversized_payload_is_rejected() {
    let bus = BroadcastBus::new(BusConfig {
        max_inline_payload_bytes: 16,
        ..BusConfig::default()
    });
    let big = Event::new(
        EventKind::Tool,
        Level::Info,
        serde_json::json!("x".repeat(100)),
    );
    let err = bus.publish(big).await.unwrap_err();
    assert!(matches!(err, BusError::PayloadTooLarge { limit: 16, .. }));
    assert_eq!(bus.stats().published, 0);
}
