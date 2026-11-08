//! Graf oczekiwania (wait-for) i wykrywanie zakleszczeń: cykl → błąd dla najmłodszego żądania.

use std::collections::{BTreeMap, BTreeSet};

use crate::table::{Effect, LockTable};
use crate::types::{Holder, RequestId};

/// Krawędź: `waiter` czeka (żądaniem `request`) na zasób trzymany/zarezerwowany przez `holder`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WaitEdge {
    /// Czekająca.
    pub waiter: Holder,
    /// Blokująca.
    pub holder: Holder,
    /// Żądanie czekającej.
    pub request: RequestId,
}

impl LockTable {
    /// Krawędzie grafu oczekiwania (bez pętli własnych — dwa zadania tej samej posiadaczki
    /// nie blokują się wzajemnie na stałe).
    pub fn wait_for_edges(&self) -> Vec<WaitEdge> {
        let mut edges = Vec::new();
        for (id, p) in &self.pending {
            let Some(slot) = self.slots.get(&p.resource) else {
                continue;
            };
            let lease_holder = slot
                .lease
                .and_then(|l| self.leases.get(&l))
                .map(|h| h.info.holder.clone());
            let reserved = slot.reservation.as_ref().map(|r| r.holder.clone());
            for blocker in lease_holder.into_iter().chain(reserved) {
                if blocker != p.view.holder {
                    edges.push(WaitEdge {
                        waiter: p.view.holder.clone(),
                        holder: blocker,
                        request: *id,
                    });
                }
            }
        }
        edges.sort();
        edges
    }

    /// Cykl w grafie oczekiwania (krawędzie cyklu), jeśli istnieje.
    pub fn find_cycle(&self) -> Option<Vec<WaitEdge>> {
        let edges = self.wait_for_edges();
        let mut adjacency: BTreeMap<&Holder, Vec<&WaitEdge>> = BTreeMap::new();
        for edge in &edges {
            adjacency.entry(&edge.waiter).or_default().push(edge);
        }
        let mut done: BTreeSet<&Holder> = BTreeSet::new();
        for start in adjacency.keys() {
            let mut path: Vec<&WaitEdge> = Vec::new();
            if let Some(cycle) = dfs(start, &adjacency, &mut path, &mut done) {
                return Some(cycle.into_iter().cloned().collect());
            }
        }
        None
    }

    /// Usuwa zakleszczenia: dopóki jest cykl, odrzuca najmłodsze żądanie w cyklu.
    pub(crate) fn resolve_deadlocks(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        while let Some(cycle) = self.find_cycle() {
            let Some(youngest) = cycle.iter().map(|e| e.request).max() else {
                break;
            };
            let holders: Vec<Holder> = cycle.iter().map(|e| e.waiter.clone()).collect();
            let Some(p) = self.remove_pending(youngest) else {
                break;
            };
            effects.push(Effect::Deadlock {
                request: p.view,
                resource: p.resource,
                cycle: holders,
            });
        }
        effects
    }
}

/// DFS z wykrywaniem powrotu na ścieżkę; zwraca krawędzie cyklu.
fn dfs<'a>(
    node: &'a Holder,
    adjacency: &BTreeMap<&'a Holder, Vec<&'a WaitEdge>>,
    path: &mut Vec<&'a WaitEdge>,
    done: &mut BTreeSet<&'a Holder>,
) -> Option<Vec<&'a WaitEdge>> {
    if done.contains(node) {
        return None;
    }
    for edge in adjacency.get(node).map(Vec::as_slice).unwrap_or_default() {
        if let Some(pos) = path.iter().position(|e| e.waiter == edge.holder) {
            let mut cycle: Vec<&WaitEdge> = path[pos..].to_vec();
            cycle.push(edge);
            return Some(cycle);
        }
        if edge.holder == *node {
            continue;
        }
        path.push(edge);
        if let Some(cycle) = dfs(&edge.holder, adjacency, path, done) {
            return Some(cycle);
        }
        path.pop();
    }
    done.insert(node);
    None
}
