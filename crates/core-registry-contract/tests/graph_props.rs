//! Testy własności grafu: dowolny DAG → kolejność spełnia zależności; dodany cykl → błąd.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use core_registry_contract::{DependencyGraph, ModuleManifest, RegistryError};
use proptest::prelude::*;

/// Losowy DAG: węzeł `i` może zależeć tylko od węzłów `j < i` (bity maski).
fn dag() -> impl Strategy<Value = Vec<Vec<usize>>> {
    (1usize..14).prop_flat_map(|n| {
        proptest::collection::vec(any::<u16>(), n).prop_map(|masks| {
            masks
                .iter()
                .enumerate()
                .map(|(i, mask)| (0..i).filter(|j| mask & (1 << j) != 0).collect())
                .collect()
        })
    })
}

fn manifests(edges: &[Vec<usize>], order: &[usize]) -> Vec<ModuleManifest> {
    order
        .iter()
        .map(|&i| {
            let requires = edges[i]
                .iter()
                .map(|j| format!("\"m{j}-contract@1\""))
                .collect::<Vec<_>>()
                .join(", ");
            let text = format!(
                "id = \"m{i}\"\nversion = \"1.0.0\"\nkind = \"tool\"\n\
                 provides = [\"m{i}-contract@1\"]\nrequires = [{requires}]\n\
                 [budget]\nram_mb = 1\ncpu_pct = 1\n"
            );
            ModuleManifest::parse_toml(&text).unwrap()
        })
        .collect()
}

fn position(order: &[core_registry_contract::ModuleId], i: usize) -> usize {
    let name = format!("m{i}");
    order.iter().position(|m| m.as_str() == name).unwrap()
}

proptest! {
    #[test]
    fn dag_order_satisfies_dependencies(edges in dag(), seed in any::<u64>()) {
        let n = edges.len();
        // Kolejność wejścia nie może wpływać na wynik (determinizm).
        let mut shuffled: Vec<usize> = (0..n).collect();
        shuffled.sort_by_key(|i| (seed.rotate_left(*i as u32) ^ *i as u64, *i));
        let a = DependencyGraph::build(&manifests(&edges, &shuffled), &BTreeSet::new()).unwrap();
        let identity: Vec<usize> = (0..n).collect();
        let b = DependencyGraph::build(&manifests(&edges, &identity), &BTreeSet::new()).unwrap();
        prop_assert_eq!(a.order(), b.order());
        prop_assert_eq!(a.order().len(), n);
        for (i, deps) in edges.iter().enumerate() {
            for &j in deps {
                prop_assert!(position(a.order(), j) < position(a.order(), i));
            }
        }
    }

    #[test]
    fn back_edge_creates_cycle_error(edges in dag(), pick in any::<prop::sample::Index>()) {
        // Dokładamy łańcuch i → i-1, żeby istniała ścieżka, potem krawędź wsteczną.
        let n = edges.len();
        prop_assume!(n >= 2);
        let mut edges = edges;
        for (i, deps) in edges.iter_mut().enumerate().skip(1) {
            if !deps.contains(&(i - 1)) {
                deps.push(i - 1);
            }
        }
        let low = pick.index(n - 1);
        edges[low].push(n - 1);
        let identity: Vec<usize> = (0..n).collect();
        let result = DependencyGraph::build(&manifests(&edges, &identity), &BTreeSet::new());
        match result {
            Err(RegistryError::Cycle(path)) => {
                prop_assert!(path.len() >= 2);
                prop_assert_eq!(path.first(), path.last());
            }
            other => prop_assert!(false, "oczekiwano cyklu, jest {:?}", other),
        }
    }
}
