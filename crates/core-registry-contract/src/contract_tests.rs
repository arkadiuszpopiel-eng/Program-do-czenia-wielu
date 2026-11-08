//! Współdzielone testy kontraktowe rejestru (feature `contract-tests`).
//! Ten sam zestaw uruchamiają `core-registry-impl` i `core-registry-fake`; rozjazd = błąd.

use std::fmt::Display;
use std::time::Duration;

use core_bus_contract::EventFilter;
use futures_util::{FutureExt, StreamExt};

pub use crate::contract_support::{Harness, StartLog, StubModule, manifest};
use crate::{
    ContractRef, HealthStatus, Lifecycle, ModuleId, ModuleManifest, ModuleState, Registry,
    RegistryError, registry::EVENT_STATE_CHANGED,
};

fn ok<T, E: Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn mid(s: &str) -> ModuleId {
    ok(ModuleId::new(s))
}

fn cref(s: &str) -> ContractRef {
    ok(s.parse())
}

fn names(ids: &[ModuleId]) -> Vec<&str> {
    ids.iter().map(ModuleId::as_str).collect()
}

fn entries(log: &StartLog) -> Vec<String> {
    log.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

async fn add<R: Registry>(r: &R, log: &StartLog, m: ModuleManifest) {
    ok(r.register(Box::new(StubModule::new(m, log.clone()))).await);
}

async fn state<R: Registry>(r: &R, id: &str) -> ModuleState {
    r.list()
        .await
        .into_iter()
        .find(|s| s.id.as_str() == id)
        .map(|s| s.state)
        .unwrap_or_else(|| panic!("brak modułu {id}"))
}

/// Zestaw a(always)→b(lazy)→c(on-demand) + d(lazy, niezależny).
async fn chain<R: Registry>(r: &R, log: &StartLog) {
    add(
        r,
        log,
        manifest("a", Lifecycle::Always, &[], &["b-contract@1"]),
    )
    .await;
    let b = manifest("b", Lifecycle::Lazy, &["b-contract@1"], &["c-contract@1"]);
    add(r, log, b).await;
    add(
        r,
        log,
        manifest("c", Lifecycle::OnDemand, &["c-contract@1"], &[]),
    )
    .await;
    add(
        r,
        log,
        manifest("d", Lifecycle::Lazy, &["d-contract@1"], &[]),
    )
    .await;
}

/// `start_order` jest topologiczny; `boot` startuje `always` z zależnościami, reszta czeka.
pub async fn boot_starts_always_in_dependency_order<R: Registry>(h: Harness<R>) {
    let log = StartLog::default();
    chain(&h.registry, &log).await;
    assert_eq!(
        names(&ok(h.registry.start_order().await)),
        ["c", "b", "a", "d"]
    );
    assert_eq!(names(&ok(h.registry.boot().await)), ["c", "b", "a"]);
    assert_eq!(entries(&log), ["start:c", "start:b", "start:a"]);
    assert_eq!(state(&h.registry, "a").await, ModuleState::Ready);
    assert_eq!(state(&h.registry, "d").await, ModuleState::Unloaded);
    assert!(
        ok(h.registry.boot().await).is_empty(),
        "boot jest idempotentny"
    );
}

/// Brak kontraktu, cykl, konflikt i duplikat → błąd (nie panika); nic nie startuje.
pub async fn invalid_graph_is_error<R: Registry, F: Fn() -> Harness<R>>(factory: F) {
    let log = StartLog::default();
    let h = factory();
    add(
        &h.registry,
        &log,
        manifest("a", Lifecycle::Always, &[], &["x-contract@1"]),
    )
    .await;
    let missing = h.registry.boot().await;
    assert!(matches!(
        missing,
        Err(RegistryError::MissingContract { .. })
    ));
    assert!(h.registry.start_order().await.is_err());

    let h = factory();
    add(
        &h.registry,
        &log,
        manifest("p", Lifecycle::Lazy, &["p-contract@1"], &["q-contract@1"]),
    )
    .await;
    add(
        &h.registry,
        &log,
        manifest("q", Lifecycle::Lazy, &["q-contract@1"], &["p-contract@1"]),
    )
    .await;
    assert!(matches!(
        h.registry.start_order().await,
        Err(RegistryError::Cycle(_))
    ));
    let acquired = h.registry.acquire(&cref("p-contract@1")).await;
    assert!(matches!(acquired, Err(RegistryError::Cycle(_))));

    let h = factory();
    add(
        &h.registry,
        &log,
        manifest("u", Lifecycle::Lazy, &["x-contract@1"], &[]),
    )
    .await;
    add(
        &h.registry,
        &log,
        manifest("v", Lifecycle::Lazy, &["x-contract@1"], &[]),
    )
    .await;
    let conflict = h.registry.start_order().await;
    assert!(matches!(
        conflict,
        Err(RegistryError::ConflictingProviders { .. })
    ));
    let dup = h
        .registry
        .register(Box::new(StubModule::new(
            manifest("u", Lifecycle::Lazy, &[], &[]),
            log.clone(),
        )))
        .await;
    assert_eq!(dup, Err(RegistryError::Duplicate(mid("u"))));
    assert!(entries(&log).is_empty(), "żaden moduł nie mógł wystartować");
}

/// `lazy` startuje przy pierwszym `acquire` (raz); nieznany kontrakt → `NoProvider`.
pub async fn lazy_starts_on_first_use<R: Registry>(h: Harness<R>) {
    let log = StartLog::default();
    chain(&h.registry, &log).await;
    assert_eq!(
        ok(h.registry.acquire(&cref("d-contract@1")).await),
        mid("d")
    );
    assert_eq!(
        ok(h.registry.acquire(&cref("d-contract@1")).await),
        mid("d")
    );
    assert_eq!(entries(&log), ["start:d"]);
    assert_eq!(state(&h.registry, "d").await, ModuleState::Ready);
    let none = h.registry.acquire(&cref("zzz-contract@1")).await;
    assert_eq!(none, Err(RegistryError::NoProvider(cref("zzz-contract@1"))));
}

/// `on-demand` wymaga `activate`; potem `acquire` działa.
pub async fn on_demand_requires_activation<R: Registry>(h: Harness<R>) {
    let log = StartLog::default();
    chain(&h.registry, &log).await;
    let before = h.registry.acquire(&cref("c-contract@1")).await;
    assert_eq!(before, Err(RegistryError::NotActivated(mid("c"))));
    ok(h.registry.activate(&mid("c")).await);
    assert_eq!(
        ok(h.registry.acquire(&cref("c-contract@1")).await),
        mid("c")
    );
    assert_eq!(entries(&log), ["start:c"]);
}

/// Zwalnianie po bezczynności: po limicie, zależne przed zależnościami, `always` nigdy.
pub async fn idle_unload_respects_timeout<R: Registry>(h: Harness<R>) {
    let log = StartLog::default();
    let r = &h.registry;
    add(r, &log, manifest("k", Lifecycle::Always, &[], &[])).await;
    add(
        r,
        &log,
        manifest("x", Lifecycle::Lazy, &["x-contract@1"], &["y-contract@1"]),
    )
    .await;
    add(
        r,
        &log,
        manifest("y", Lifecycle::Lazy, &["y-contract@1"], &[]),
    )
    .await;
    ok(r.boot().await);
    ok(r.acquire(&cref("x-contract@1")).await);
    let second = Duration::from_secs(1);
    (h.advance)(h.idle_timeout.saturating_sub(second));
    assert!(
        ok(r.unload_idle().await).is_empty(),
        "przed limitem nic nie znika"
    );
    ok(r.acquire(&cref("y-contract@1")).await);
    (h.advance)(second * 2);
    assert_eq!(names(&ok(r.unload_idle().await)), ["x"], "y użyty niedawno");
    (h.advance)(h.idle_timeout);
    assert_eq!(names(&ok(r.unload_idle().await)), ["y"]);
    assert_eq!(
        state(r, "k").await,
        ModuleState::Ready,
        "always nie jest zwalniany"
    );
    assert_eq!(state(r, "x").await, ModuleState::Unloaded);
}

/// `deactivate` zatrzymuje najpierw zależne; `always` → `Resident`; health wg stanu.
pub async fn deactivate_and_health<R: Registry>(h: Harness<R>) {
    let log = StartLog::default();
    let r = &h.registry;
    add(
        r,
        &log,
        manifest("x", Lifecycle::Lazy, &["x-contract@1"], &["y-contract@1"]),
    )
    .await;
    add(
        r,
        &log,
        manifest("y", Lifecycle::Lazy, &["y-contract@1"], &[]),
    )
    .await;
    add(r, &log, manifest("k", Lifecycle::Always, &[], &[])).await;
    ok(r.activate(&mid("x")).await);
    assert_eq!(ok(r.health(&mid("x")).await), HealthStatus::Healthy);
    assert_eq!(names(&ok(r.deactivate(&mid("y")).await)), ["x", "y"]);
    assert_eq!(entries(&log), ["start:y", "start:x", "stop:x", "stop:y"]);
    assert_eq!(ok(r.health(&mid("x")).await), HealthStatus::NotStarted);
    assert!(ok(r.deactivate(&mid("y")).await).is_empty());
    ok(r.boot().await);
    assert_eq!(
        r.deactivate(&mid("k")).await,
        Err(RegistryError::Resident(mid("k")))
    );
    let unknown = r.health(&mid("nope")).await;
    assert_eq!(unknown, Err(RegistryError::UnknownModule(mid("nope"))));
}

/// Nieudany start → `StartFailed` i stan `Failed { restarts: 1 }`.
pub async fn start_failure_marks_failed<R: Registry>(h: Harness<R>) {
    let log = StartLog::default();
    let m = manifest("f", Lifecycle::OnDemand, &["f-contract@1"], &[]);
    ok(h.registry
        .register(Box::new(StubModule::failing(m, log.clone())))
        .await);
    let err = h.registry.activate(&mid("f")).await;
    assert!(matches!(err, Err(RegistryError::StartFailed { .. })));
    match state(&h.registry, "f").await {
        ModuleState::Failed { restarts, .. } => assert_eq!(restarts, 1),
        other => panic!("oczekiwano Failed: {other:?}"),
    }
}

/// Wyłączenie: moduł wymagany przez włączone → `InUse`; wyłączony nie startuje.
pub async fn disable_and_enable<R: Registry>(h: Harness<R>) {
    let log = StartLog::default();
    let r = &h.registry;
    add(
        r,
        &log,
        manifest("x", Lifecycle::Lazy, &["x-contract@1"], &["y-contract@1"]),
    )
    .await;
    add(
        r,
        &log,
        manifest("y", Lifecycle::Lazy, &["y-contract@1"], &[]),
    )
    .await;
    ok(r.activate(&mid("x")).await);
    let in_use = r.set_enabled(&mid("y"), false).await;
    assert!(matches!(in_use, Err(RegistryError::InUse { .. })));
    ok(r.set_enabled(&mid("x"), false).await);
    assert_eq!(state(r, "x").await, ModuleState::Disabled);
    assert_eq!(
        r.activate(&mid("x")).await,
        Err(RegistryError::Disabled(mid("x")))
    );
    let gone = r.acquire(&cref("x-contract@1")).await;
    assert_eq!(gone, Err(RegistryError::NoProvider(cref("x-contract@1"))));
    ok(r.set_enabled(&mid("x"), true).await);
    assert_eq!(state(r, "x").await, ModuleState::Unloaded);
    assert_eq!(names(&ok(r.shutdown().await)), ["y"]);
    assert_eq!(entries(&log), ["start:y", "start:x", "stop:x", "stop:y"]);
}

/// Każda zmiana stanu trafia na magistralę jako `registry.module.state_changed`.
pub async fn state_changes_are_published<R: Registry>(h: Harness<R>) {
    let log = StartLog::default();
    let mut sub = ok(h
        .bus
        .subscribe(EventFilter::prefix(EVENT_STATE_CHANGED))
        .await);
    add(
        &h.registry,
        &log,
        manifest("y", Lifecycle::Lazy, &["y-contract@1"], &[]),
    )
    .await;
    ok(h.registry.acquire(&cref("y-contract@1")).await);
    ok(h.registry.deactivate(&mid("y")).await);
    let mut seen = Vec::new();
    while let Some(Some(item)) = sub.next().now_or_never() {
        if let Some(ev) = item.event() {
            let p = &ev.payload;
            seen.push(format!("{}:{}>{}", p["module"], p["from"], p["to"]).replace('"', ""));
        }
    }
    assert_eq!(
        seen,
        ["y:unloaded>loading", "y:loading>ready", "y:ready>unloaded"]
    );
}

/// Uruchamia cały zestaw; `factory` daje świeże środowisko dla każdego przypadku.
pub async fn run_all<R: Registry, F: Fn() -> Harness<R>>(factory: F) {
    boot_starts_always_in_dependency_order(factory()).await;
    invalid_graph_is_error(&factory).await;
    lazy_starts_on_first_use(factory()).await;
    on_demand_requires_activation(factory()).await;
    idle_unload_respects_timeout(factory()).await;
    deactivate_and_health(factory()).await;
    start_failure_marks_failed(factory()).await;
    disable_and_enable(factory()).await;
    state_changes_are_published(factory()).await;
}
