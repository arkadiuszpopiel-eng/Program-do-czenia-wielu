//! Testy własności rejestru: dowolny DAG manifestów → start i stop zgodne z zależnościami;
//! graf z cyklem → błąd bez uruchamiania czegokolwiek.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use core_bus_fake::FakeBus;
use core_registry_contract::contract_tests::{StartLog, StubModule, manifest};
use core_registry_contract::{Lifecycle, Registry, RegistryError};
use core_registry_impl::ModuleRegistry;
use proptest::prelude::*;

/// Węzeł `i`: cykl życia + zależności od węzłów `j < i`.
fn dag() -> impl Strategy<Value = Vec<(Lifecycle, Vec<usize>)>> {
    let lifecycle = prop_oneof![
        Just(Lifecycle::Always),
        Just(Lifecycle::Lazy),
        Just(Lifecycle::OnDemand)
    ];
    proptest::collection::vec((lifecycle, any::<u16>()), 1..12).prop_map(|nodes| {
        nodes
            .into_iter()
            .enumerate()
            .map(|(i, (l, mask))| (l, (0..i).filter(|j| mask & (1 << j) != 0).collect()))
            .collect()
    })
}

fn contract(i: usize) -> String {
    format!("m{i}-contract@1")
}

async fn build(nodes: &[(Lifecycle, Vec<usize>)], log: &StartLog) -> ModuleRegistry {
    let registry = ModuleRegistry::new(Arc::new(FakeBus::default()));
    // Rejestracja w odwrotnej kolejności — wynik nie może od niej zależeć.
    for (i, (lifecycle, deps)) in nodes.iter().enumerate().rev() {
        let provides = [contract(i)];
        let requires: Vec<String> = deps.iter().map(|j| contract(*j)).collect();
        let requires: Vec<&str> = requires.iter().map(String::as_str).collect();
        let provides: Vec<&str> = provides.iter().map(String::as_str).collect();
        let m = manifest(&format!("m{i}"), *lifecycle, &provides, &requires);
        registry
            .register(Box::new(StubModule::new(m, log.clone())))
            .await
            .unwrap();
    }
    registry
}

fn index_of(log: &[String], entry: &str) -> Option<usize> {
    log.iter().position(|e| e == entry)
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
}

proptest! {
    #[test]
    fn boot_and_shutdown_respect_dependencies(nodes in dag()) {
        runtime().block_on(async {
            let log = StartLog::default();
            let registry = build(&nodes, &log).await;
            let order = registry.start_order().await.unwrap();
            assert_eq!(order.len(), nodes.len());
            registry.boot().await.unwrap();
            let started = log.lock().unwrap().clone();
            for (i, (lifecycle, deps)) in nodes.iter().enumerate() {
                let me = index_of(&started, &format!("start:m{i}"));
                if *lifecycle == Lifecycle::Always {
                    assert!(me.is_some(), "always m{i} nie wystartował");
                }
                if let Some(me) = me {
                    for j in deps {
                        let dep = index_of(&started, &format!("start:m{j}"));
                        assert!(dep.is_some_and(|d| d < me), "m{j} po m{i}");
                    }
                }
            }
            log.lock().unwrap().clear();
            registry.shutdown().await.unwrap();
            let stopped = log.lock().unwrap().clone();
            assert_eq!(stopped.len(), started.len());
            for (i, (_, deps)) in nodes.iter().enumerate() {
                if let Some(me) = index_of(&stopped, &format!("stop:m{i}")) {
                    for j in deps {
                        let dep = index_of(&stopped, &format!("stop:m{j}")).unwrap();
                        assert!(me < dep, "m{j} zatrzymany przed zależnym m{i}");
                    }
                }
            }
        });
    }

    #[test]
    fn cycle_is_error_and_nothing_starts(nodes in dag(), pick in any::<prop::sample::Index>()) {
        prop_assume!(nodes.len() >= 2);
        let mut nodes = nodes;
        let n = nodes.len();
        for (i, (_, deps)) in nodes.iter_mut().enumerate().skip(1) {
            if !deps.contains(&(i - 1)) {
                deps.push(i - 1);
            }
        }
        nodes[pick.index(n - 1)].1.push(n - 1);
        nodes[0].0 = Lifecycle::Always;
        runtime().block_on(async {
            let log = StartLog::default();
            let registry = build(&nodes, &log).await;
            assert!(matches!(registry.start_order().await, Err(RegistryError::Cycle(_))));
            assert!(matches!(registry.boot().await, Err(RegistryError::Cycle(_))));
            assert!(log.lock().unwrap().is_empty());
        });
    }
}
