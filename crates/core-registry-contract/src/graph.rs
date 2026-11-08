//! Graf zależności kontraktów: walidacja (braki, konflikty, cykle) i kolejność startu.
//! Czysta funkcja dzielona przez `core-registry-impl` i `core-registry-fake`, żeby reguły
//! rozwiązywania nie rozjechały się między implementacją a atrapą.

use std::collections::{BTreeMap, BTreeSet};

use crate::manifest::ModuleManifest;
use crate::refs::{ContractRef, ModuleId};
use crate::registry::RegistryError;

/// Zwalidowany graf: dostawcy kontraktów, krawędzie i kolejność topologiczna.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyGraph {
    order: Vec<ModuleId>,
    deps: BTreeMap<ModuleId, BTreeSet<ModuleId>>,
    providers: BTreeMap<ContractRef, ModuleId>,
}

impl DependencyGraph {
    /// Buduje graf z manifestów (tylko moduły włączone). `external` to kontrakty dostarczane
    /// przez jądro poza rejestrem (np. `core-bus-contract@1`) — spełniają wymagania, a moduł
    /// dostarczający taki kontrakt to konflikt.
    ///
    /// Kolejność startu: zależności przed zależnymi, remisy rozstrzygane leksykograficznie po `id`
    /// (deterministycznie). Błędy: `Duplicate`, `ConflictingProviders`, `MissingContract`, `Cycle`.
    pub fn build<'a>(
        manifests: impl IntoIterator<Item = &'a ModuleManifest>,
        external: &BTreeSet<ContractRef>,
    ) -> Result<Self, RegistryError> {
        let mut by_id: BTreeMap<ModuleId, &ModuleManifest> = BTreeMap::new();
        for m in manifests {
            if by_id.insert(m.id.clone(), m).is_some() {
                return Err(RegistryError::Duplicate(m.id.clone()));
            }
        }
        let providers = collect_providers(&by_id, external)?;
        let mut deps: BTreeMap<ModuleId, BTreeSet<ModuleId>> = BTreeMap::new();
        for (id, m) in &by_id {
            let mut set = BTreeSet::new();
            for req in &m.requires {
                match providers.get(req) {
                    Some(provider) => {
                        set.insert(provider.clone());
                    }
                    None if external.contains(req) => {}
                    None => {
                        return Err(RegistryError::MissingContract {
                            module: id.clone(),
                            contract: req.clone(),
                        });
                    }
                }
            }
            deps.insert(id.clone(), set);
        }
        let order = topological_order(&deps)?;
        Ok(Self {
            order,
            deps,
            providers,
        })
    }

    /// Kolejność startu wszystkich modułów grafu.
    pub fn order(&self) -> &[ModuleId] {
        &self.order
    }

    /// Dostawca kontraktu (moduł), jeśli jest w grafie.
    pub fn provider(&self, contract: &ContractRef) -> Option<&ModuleId> {
        self.providers.get(contract)
    }

    /// Bezpośrednie zależności modułu (moduły dostarczające jego `requires`).
    pub fn dependencies(&self, id: &ModuleId) -> Vec<ModuleId> {
        self.deps
            .get(id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Bezpośrednio zależne moduły (wymagające kontraktu dostarczanego przez `id`).
    pub fn dependents(&self, id: &ModuleId) -> Vec<ModuleId> {
        self.deps
            .iter()
            .filter(|(_, d)| d.contains(id))
            .map(|(m, _)| m.clone())
            .collect()
    }

    /// `id` i wszystkie jego zależności przechodnie — w kolejności startu (`id` na końcu).
    pub fn start_closure(&self, id: &ModuleId) -> Vec<ModuleId> {
        let mut needed = BTreeSet::new();
        let mut stack = vec![id.clone()];
        while let Some(m) = stack.pop() {
            if needed.insert(m.clone()) {
                stack.extend(self.dependencies(&m));
            }
        }
        self.order
            .iter()
            .filter(|m| needed.contains(*m))
            .cloned()
            .collect()
    }

    /// `id` i wszystkie moduły zależne przechodnio — w kolejności zatrzymania (`id` na końcu).
    pub fn stop_closure(&self, id: &ModuleId) -> Vec<ModuleId> {
        let mut affected = BTreeSet::new();
        let mut stack = vec![id.clone()];
        while let Some(m) = stack.pop() {
            if affected.insert(m.clone()) {
                stack.extend(self.dependents(&m));
            }
        }
        self.order
            .iter()
            .rev()
            .filter(|m| affected.contains(*m))
            .cloned()
            .collect()
    }
}

fn collect_providers(
    by_id: &BTreeMap<ModuleId, &ModuleManifest>,
    external: &BTreeSet<ContractRef>,
) -> Result<BTreeMap<ContractRef, ModuleId>, RegistryError> {
    let mut all: BTreeMap<ContractRef, Vec<ModuleId>> = BTreeMap::new();
    for (id, m) in by_id {
        for p in &m.provides {
            all.entry(p.clone()).or_default().push(id.clone());
        }
    }
    let mut providers = BTreeMap::new();
    for (contract, modules) in all {
        if modules.len() > 1 || external.contains(&contract) {
            return Err(RegistryError::ConflictingProviders { contract, modules });
        }
        if let Some(first) = modules.into_iter().next() {
            providers.insert(contract, first);
        }
    }
    Ok(providers)
}

/// Algorytm Kahna z kolejką uporządkowaną po `id`; pozostałe węzły → opis cyklu.
fn topological_order(
    deps: &BTreeMap<ModuleId, BTreeSet<ModuleId>>,
) -> Result<Vec<ModuleId>, RegistryError> {
    let mut remaining: BTreeMap<&ModuleId, usize> =
        deps.iter().map(|(id, d)| (id, d.len())).collect();
    let mut ready: BTreeSet<&ModuleId> = remaining
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order = Vec::with_capacity(deps.len());
    while let Some(next) = ready.pop_first() {
        remaining.remove(next);
        order.push(next.clone());
        for (dependent, d) in deps {
            if d.contains(next)
                && let Some(n) = remaining.get_mut(dependent)
            {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    ready.insert(dependent);
                }
            }
        }
    }
    if remaining.is_empty() {
        return Ok(order);
    }
    let stuck: BTreeSet<&ModuleId> = remaining.keys().copied().collect();
    Err(RegistryError::Cycle(find_cycle(deps, &stuck)))
}

/// Idzie po zależnościach wewnątrz węzłów bez kolejności, aż trafi na powtórzenie.
fn find_cycle(
    deps: &BTreeMap<ModuleId, BTreeSet<ModuleId>>,
    stuck: &BTreeSet<&ModuleId>,
) -> Vec<ModuleId> {
    let Some(start) = stuck.first() else {
        return Vec::new();
    };
    let mut walk: Vec<ModuleId> = vec![(*start).clone()];
    loop {
        let Some(current) = walk.last() else {
            return walk;
        };
        let next = deps
            .get(current)
            .and_then(|d| d.iter().find(|m| stuck.contains(m)));
        let Some(next) = next else {
            return walk;
        };
        if let Some(pos) = walk.iter().position(|m| m == next) {
            let mut cycle = walk.split_off(pos);
            cycle.push(next.clone());
            return cycle;
        }
        walk.push(next.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, provides: &[&str], requires: &[&str]) -> ModuleManifest {
        let list = |xs: &[&str]| {
            xs.iter()
                .map(|x| format!("\"{x}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let text = format!(
            "id = \"{id}\"\nversion = \"1.0.0\"\nkind = \"service\"\nprovides = [{}]\nrequires = [{}]\n[budget]\nram_mb = 1\ncpu_pct = 1\n",
            list(provides),
            list(requires)
        );
        ModuleManifest::parse_toml(&text).unwrap()
    }

    fn ids(xs: &[ModuleId]) -> Vec<&str> {
        xs.iter().map(ModuleId::as_str).collect()
    }

    fn none() -> BTreeSet<ContractRef> {
        BTreeSet::new()
    }

    #[test]
    fn orders_dependencies_first_with_lexicographic_ties() {
        let ms = [
            m("a", &[], &["b-contract@1"]),
            m("b", &["b-contract@1"], &["c-contract@1"]),
            m("c", &["c-contract@1"], &[]),
            m("d", &[], &[]),
        ];
        let g = DependencyGraph::build(&ms, &none()).unwrap();
        assert_eq!(ids(g.order()), ["c", "b", "a", "d"]);
        let a = ModuleId::new("a").unwrap();
        let c = ModuleId::new("c").unwrap();
        assert_eq!(ids(&g.start_closure(&a)), ["c", "b", "a"]);
        assert_eq!(ids(&g.stop_closure(&c)), ["a", "b", "c"]);
        assert_eq!(ids(&g.dependents(&c)), ["b"]);
    }

    #[test]
    fn external_contracts_satisfy_but_conflict_when_provided() {
        let ext: BTreeSet<ContractRef> = ["core-bus-contract@1".parse().unwrap()].into();
        let ok = [m("a", &[], &["core-bus-contract@1"])];
        assert!(DependencyGraph::build(&ok, &ext).is_ok());
        let bad = [m("a", &["core-bus-contract@1"], &[])];
        assert!(matches!(
            DependencyGraph::build(&bad, &ext),
            Err(RegistryError::ConflictingProviders { .. })
        ));
    }

    #[test]
    fn major_version_must_match() {
        let ms = [
            m("a", &[], &["b-contract@2"]),
            m("b", &["b-contract@1"], &[]),
        ];
        let err = DependencyGraph::build(&ms, &none()).unwrap_err();
        assert!(matches!(err, RegistryError::MissingContract { .. }));
    }

    #[test]
    fn detects_duplicates_conflicts_and_cycles() {
        let dup = [m("a", &[], &[]), m("a", &[], &[])];
        assert!(matches!(
            DependencyGraph::build(&dup, &none()),
            Err(RegistryError::Duplicate(_))
        ));
        let conflict = [
            m("a", &["x-contract@1"], &[]),
            m("b", &["x-contract@1"], &[]),
        ];
        assert!(matches!(
            DependencyGraph::build(&conflict, &none()),
            Err(RegistryError::ConflictingProviders { .. })
        ));
        let cycle = [
            m("a", &["a-contract@1"], &["b-contract@1"]),
            m("b", &["b-contract@1"], &["c-contract@1"]),
            m("c", &["c-contract@1"], &["a-contract@1"]),
            m("z", &[], &["a-contract@1"]),
        ];
        match DependencyGraph::build(&cycle, &none()) {
            Err(RegistryError::Cycle(path)) => assert_eq!(ids(&path), ["a", "b", "c", "a"]),
            other => panic!("oczekiwano cyklu: {other:?}"),
        }
    }
}
