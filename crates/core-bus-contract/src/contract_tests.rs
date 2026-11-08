//! Współdzielone testy kontraktowe magistrali (feature `contract-tests`).
//! Uruchamiane w testach `core-bus-impl` i `core-bus-fake`; rozjazd zachowań = błąd.

use std::time::Duration;

use futures_util::StreamExt;

use crate::{BusItem, Event, EventBus, EventFilter, EventKind, Level, SessionId};

const TIMEOUT: Duration = Duration::from_secs(2);

async fn next_event<S: futures_core::Stream<Item = BusItem> + Unpin>(stream: &mut S) -> Event {
    loop {
        let item = tokio::time::timeout(TIMEOUT, stream.next())
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| panic!("brak zdarzenia w {TIMEOUT:?}"));
        if let BusItem::Event(ev) = item {
            return (*ev).clone();
        }
    }
}

/// Zdarzenie opublikowane trafia do subskrybenta z tym samym rodzajem, poziomem i ładunkiem.
pub async fn publish_reaches_subscriber<B: EventBus>(bus: &B) {
    let mut sub = bus
        .subscribe(EventFilter::all())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let ev = Event::new(EventKind::Tool, Level::Info, serde_json::json!({"step": 1}));
    bus.publish(ev.clone())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let got = next_event(&mut sub).await;
    assert_eq!(got.kind, ev.kind);
    assert_eq!(got.level, ev.level);
    assert_eq!(got.payload, ev.payload);
}

/// Filtr po rodzaju i sesji pomija zdarzenia niepasujące.
pub async fn filter_is_applied<B: EventBus>(bus: &B) {
    let filter = EventFilter::kind(EventKind::Voice).with_session(SessionId::from("s1"));
    let mut sub = bus
        .subscribe(filter)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let noise = Event::new(EventKind::Tool, Level::Info, serde_json::json!("noise"))
        .with_session(SessionId::from("s1"));
    let other_session = Event::new(EventKind::Voice, Level::Info, serde_json::json!("s2"))
        .with_session(SessionId::from("s2"));
    let wanted = Event::new(EventKind::Voice, Level::Info, serde_json::json!("wanted"))
        .with_session(SessionId::from("s1"));
    for ev in [noise, other_session, wanted] {
        bus.publish(ev).await.unwrap_or_else(|e| panic!("{e}"));
    }
    let got = next_event(&mut sub).await;
    assert_eq!(got.payload, serde_json::json!("wanted"));
}

/// Publikacja bez subskrybentów nie jest błędem, a licznik `published` rośnie.
pub async fn publish_without_subscribers_is_ok<B: EventBus>(bus: &B) {
    let before = bus.stats().published;
    bus.publish(Event::new(
        EventKind::Ui,
        Level::Debug,
        serde_json::Value::Null,
    ))
    .await
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(bus.stats().published, before + 1);
}

/// Kolejność zdarzeń jednego wydawcy jest zachowana u subskrybenta.
pub async fn order_is_preserved_for_single_publisher<B: EventBus>(bus: &B) {
    let mut sub = bus
        .subscribe(EventFilter::prefix("order."))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    for i in 0..20u64 {
        let ev = Event::new(
            EventKind::Custom("order.step".into()),
            Level::Info,
            i.into(),
        );
        bus.publish(ev).await.unwrap_or_else(|e| panic!("{e}"));
    }
    for i in 0..20u64 {
        assert_eq!(
            next_event(&mut sub).await.payload,
            serde_json::Value::from(i)
        );
    }
}

/// Uruchamia cały zestaw kontraktowy na świeżych instancjach z fabryki.
pub async fn run_all<B: EventBus, F: Fn() -> B>(factory: F) {
    publish_reaches_subscriber(&factory()).await;
    filter_is_applied(&factory()).await;
    publish_without_subscribers_is_ok(&factory()).await;
    order_is_preserved_for_single_publisher(&factory()).await;
}
