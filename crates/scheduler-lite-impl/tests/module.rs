//! Testy implementacji: kontrakt współdzielony (zatrzymany zegar tokio), manifest, cykl życia,
//! zdarzenia na magistrali, 100 współbieżnych scenariuszy bez nakładania się (ACC F2-12),
//! wywłaszczenie przez mowę użytkownika ≤ 5 ms (ACC-F2-scheduler-lite-02).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use personas_contract::PersonaId;
use scheduler_lite_contract::contract_tests::{self, Harness, poll_once};
use scheduler_lite_contract::{
    EVENT_GRANTED, EVENT_RELEASED, EVENT_TIMEOUT, Holder, LeaseRequest, OnTimeout, Priority,
    Resource, ResourcePolicy, SchedError, SchedulerLite, event_kind,
};
use scheduler_lite_impl::{MODULE_TOML, SchedulerModule};

async fn started() -> (SchedulerModule, FakeBus) {
    let bus = FakeBus::default();
    let mut module = SchedulerModule::new().unwrap();
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    (module, bus)
}

async fn settle() {
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

struct ImplHarness(SchedulerModule);

#[async_trait]
impl Harness for ImplHarness {
    type S = SchedulerModule;
    fn scheduler(&self) -> &SchedulerModule {
        &self.0
    }
    async fn advance(&self, ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        settle().await;
    }
}

fn persona(name: &str) -> Holder {
    Holder::Persona(PersonaId::new(name))
}

#[tokio::test(start_paused = true)]
async fn contract_suite() {
    contract_tests::run_all(|| async { ImplHarness(started().await.0) }).await;
}

#[test]
fn manifest_is_valid() {
    let module = SchedulerModule::new().unwrap();
    let m = module.manifest();
    assert_eq!(m.id.as_str(), "scheduler-lite");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.lifecycle, Lifecycle::Always);
    assert_eq!(m.provides[0].to_string(), "scheduler-lite-contract@1");
    assert!(MODULE_TOML.contains("personas-contract@1"));
}

#[tokio::test(start_paused = true)]
async fn lifecycle_and_stop_revokes() {
    let mut module = SchedulerModule::new().unwrap();
    let req = LeaseRequest::new(
        Resource::Speaker,
        persona("alfa"),
        Priority::Normal,
        Duration::from_secs(1),
    );
    assert_eq!(module.health(), HealthStatus::NotStarted);
    assert_eq!(
        module.acquire(req.clone()).await.map(|_| ()),
        Err(SchedError::NotStarted)
    );
    assert_eq!(module.kill_all(), 0);
    assert_eq!(module.stop().await, Err(ModuleError::NotStarted));
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(FakeBus::default()));
    module.start(ctx.clone()).await.unwrap();
    assert_eq!(module.start(ctx).await, Err(ModuleError::AlreadyStarted));
    assert_eq!(module.health(), HealthStatus::Healthy);
    let lease = module.acquire(req).await.unwrap();
    module.stop().await.unwrap();
    assert!(lease.is_revoked());
    drop(lease); // rdzeń już nie istnieje — drop nic nie robi
    assert_eq!(module.health(), HealthStatus::NotStarted);
}

#[tokio::test(start_paused = true)]
async fn events_reach_bus_and_policy_applies() {
    let (module, bus) = started().await;
    module.set_policy(
        Resource::Mic,
        ResourcePolicy {
            preemptible_at_atomic: false,
            on_timeout: OnTimeout::AskUser,
            handoff_reserve_ms: 500,
        },
    );
    let held = module
        .acquire(LeaseRequest::new(
            Resource::Mic,
            persona("beta"),
            Priority::Normal,
            Duration::from_secs(1),
        ))
        .await
        .unwrap();
    let waiting = module
        .acquire(LeaseRequest::new(
            Resource::Mic,
            Holder::User,
            Priority::Normal,
            Duration::from_millis(300),
        ))
        .await;
    assert!(matches!(
        waiting,
        Err(SchedError::Timeout {
            on_timeout: OnTimeout::AskUser,
            waited_ms: 300,
            ..
        })
    ));
    drop(held);
    settle().await;
    let kinds: Vec<String> = bus.recorded().iter().map(|e| e.kind.to_string()).collect();
    assert!(
        kinds.contains(&EVENT_GRANTED.to_owned()) && kinds.contains(&EVENT_RELEASED.to_owned())
    );
    let timeout = bus.recorded_of_kind(&event_kind(EVENT_TIMEOUT));
    assert_eq!(timeout[0].payload["on_timeout"], "ask_user");
}

/// Prosty deterministyczny generator (bez zewnętrznych zależności).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

const RESOURCES: [Resource; 3] = [Resource::Speaker, Resource::ScreenInput, Resource::Mic];

async fn hold(lease: &scheduler_lite_contract::Lease, ms: u64) {
    // Punkty atomowe co 10 ms: posiadaczka sprawdza sygnał wywłaszczenia.
    let mut t = 0;
    while t < ms && !lease.preempt_requested() && !lease.is_revoked() {
        tokio::time::sleep(Duration::from_millis(10)).await;
        t += 10;
    }
}

/// Liczniki scenariusza: [przyznane, timeout, zakleszczenie].
type Stats = Arc<[AtomicUsize; 3]>;

fn count(stats: &Stats, result: &Result<scheduler_lite_contract::Lease, SchedError>) {
    let i = match result {
        Ok(_) => 0,
        Err(SchedError::Timeout { .. }) => 1,
        Err(SchedError::Deadlock { .. }) => 2,
        Err(e) => panic!("nieoczekiwany błąd: {e}"),
    };
    stats[i].fetch_add(1, Ordering::SeqCst);
}

async fn worker(
    module: Arc<SchedulerModule>,
    busy: Arc<[AtomicBool; 3]>,
    overlaps: Arc<AtomicUsize>,
    stats: Stats,
    seed: u64,
    who: Holder,
) {
    let mut rng = Lcg(seed);
    let priorities = [
        Priority::Narration,
        Priority::Normal,
        Priority::Interactive,
        Priority::UserSpeech,
    ];
    for _ in 0..10 {
        let r = usize::try_from(rng.next(3)).unwrap();
        let req = LeaseRequest::new(
            RESOURCES[r].clone(),
            who.clone(),
            priorities[usize::try_from(rng.next(4)).unwrap()],
            Duration::from_millis(rng.next(300)),
        );
        let first = module.acquire(req).await;
        count(&stats, &first);
        let Ok(first) = first else { continue };
        if busy[r].swap(true, Ordering::SeqCst) {
            overlaps.fetch_add(1, Ordering::SeqCst);
        }
        // Czasem druga, zagnieżdżona dzierżawa (możliwe zakleszczenia → błąd dla najmłodszego).
        let r2 = (r + 1 + usize::try_from(rng.next(2)).unwrap()) % 3;
        let second = if rng.next(2) == 0 {
            let req2 = LeaseRequest::new(
                RESOURCES[r2].clone(),
                who.clone(),
                Priority::Normal,
                Duration::from_millis(rng.next(200)),
            );
            let second = module.acquire(req2).await;
            count(&stats, &second);
            second.ok()
        } else {
            None
        };
        if second.is_some() && busy[r2].swap(true, Ordering::SeqCst) {
            overlaps.fetch_add(1, Ordering::SeqCst);
        }
        hold(&first, rng.next(80)).await;
        if second.is_some() {
            busy[r2].store(false, Ordering::SeqCst);
        }
        drop(second);
        busy[r].store(false, Ordering::SeqCst);
        drop(first);
    }
}

#[tokio::test(start_paused = true)]
async fn hundred_concurrent_scenarios_without_overlap() {
    let stats: Stats = Arc::new([
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    ]);
    for seed in 0..100u64 {
        let (module, _bus) = started().await;
        let module = Arc::new(module);
        let busy = Arc::new([
            AtomicBool::new(false),
            AtomicBool::new(false),
            AtomicBool::new(false),
        ]);
        let overlaps = Arc::new(AtomicUsize::new(0));
        let holders = [
            persona("alfa"),
            persona("beta"),
            persona("gama"),
            persona("delta"),
            Holder::User,
        ];
        let tasks: Vec<_> = holders
            .into_iter()
            .enumerate()
            .map(|(i, who)| {
                tokio::spawn(worker(
                    Arc::clone(&module),
                    Arc::clone(&busy),
                    Arc::clone(&overlaps),
                    Arc::clone(&stats),
                    seed * 7 + i as u64,
                    who,
                ))
            })
            .collect();
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(
            overlaps.load(Ordering::SeqCst),
            0,
            "scenariusz {seed}: nakładanie"
        );
        for r in &RESOURCES {
            assert_eq!(
                module.holder(r),
                None,
                "scenariusz {seed}: niezwolniona dzierżawa"
            );
            assert!(
                module.queue(r).is_empty(),
                "scenariusz {seed}: ktoś nadal czeka"
            );
        }
    }
    let [granted, timeouts, deadlocks] = [0, 1, 2].map(|i| stats[i].load(Ordering::SeqCst));
    eprintln!(
        "100 scenariuszy: przyznane {granted}, timeouty {timeouts}, zakleszczenia rozwiązane {deadlocks}"
    );
    assert!(
        granted > 1000 && timeouts > 0,
        "scenariusze bez rywalizacji?"
    );
}

#[tokio::test]
async fn user_speech_preemption_under_5_ms() {
    let (module, _bus) = started().await;
    let mut worst = Duration::ZERO;
    for _ in 0..100 {
        let narration = module
            .acquire(LeaseRequest::new(
                Resource::Speaker,
                persona("alfa"),
                Priority::Narration,
                Duration::from_secs(1),
            ))
            .await
            .unwrap();
        let t0 = std::time::Instant::now();
        let mut user = Box::pin(module.acquire(LeaseRequest::new(
            Resource::Speaker,
            Holder::User,
            Priority::UserSpeech,
            Duration::from_secs(5),
        )));
        assert!(poll_once(&mut user).await.is_none());
        assert!(narration.preempt_requested());
        worst = worst.max(t0.elapsed());
        drop(narration);
        drop(poll_once(&mut user).await.unwrap().unwrap());
    }
    assert!(
        worst < Duration::from_millis(5),
        "najgorsza decyzja: {worst:?}"
    );
}
