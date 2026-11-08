//! Manifest, cykl życia modułu i zdarzenia na magistrali (bez treści tur).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use sessions_contract::{NewSession, NewTurn, SessionCatalog, SessionHistory, events};
use sessions_impl::MODULE_TOML;

#[test]
fn manifest_is_valid() {
    let h = common::harness();
    let m = h.manifest();
    assert_eq!(m.id.as_str(), "sessions");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.lifecycle, Lifecycle::Always);
    assert_eq!(m.provides[0].to_string(), "sessions-contract@1");
    assert!(MODULE_TOML.contains("search-contract@1"));
}

async fn wait_for(bus: &FakeBus, kind: &str, n: usize) -> Vec<Arc<core_bus_contract::Event>> {
    let kind = EventKind::Custom(kind.to_owned());
    for _ in 0..200 {
        let got = bus.recorded_of_kind(&kind);
        if got.len() >= n {
            return got;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    bus.recorded_of_kind(&kind)
}

#[tokio::test]
async fn lifecycle_and_events() {
    let mut h = common::harness();
    assert_eq!(h.sessions.health(), HealthStatus::NotStarted);
    assert_eq!(h.sessions.stop().await, Err(ModuleError::NotStarted));
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(h.sessions.manifest().id.clone(), Arc::new(bus.clone()));
    h.sessions.start(ctx.clone()).await.unwrap();
    assert_eq!(
        h.sessions.start(ctx).await,
        Err(ModuleError::AlreadyStarted)
    );
    assert_eq!(h.sessions.health(), HealthStatus::Healthy);

    let id = h.create_session(NewSession::default()).unwrap().id;
    let u = h
        .append_turn(&id, None, NewTurn::user("tajna treść"))
        .unwrap();
    let a = h
        .append_turn(&id, Some(u.id), NewTurn::assistant("alfa", "x"))
        .unwrap();
    h.fork_from(&id, a.id, NewTurn::assistant("beta", "y"))
        .unwrap();
    h.mark_tainted(&id).unwrap();
    h.delete_session(&id).unwrap();

    assert_eq!(wait_for(&bus, events::SESSION_CREATED, 1).await.len(), 1);
    let appended = wait_for(&bus, events::TURN_APPENDED, 3).await;
    assert_eq!(appended.len(), 3);
    assert_eq!(appended[0].session.as_ref(), Some(&id));
    assert!(!appended[0].payload.to_string().contains("tajna"));
    assert_eq!(wait_for(&bus, events::BRANCH_CREATED, 1).await.len(), 1);
    assert_eq!(wait_for(&bus, events::SESSION_TAINTED, 1).await.len(), 1);
    assert_eq!(wait_for(&bus, events::SESSION_DELETED, 1).await.len(), 1);

    h.sessions.stop().await.unwrap();
    assert_eq!(h.sessions.health(), HealthStatus::NotStarted);
}
