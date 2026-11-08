//! Plan kaskady `forget` (czysta funkcja, wspólna dla `-impl` i `-fake`).
//!
//! Reguły (PLAN §10 „forget kaskadowo”, prywatność — wariant bezpieczniejszy):
//! 1. **Nasiona** — wpisy wskazane przez cel (wpis, zakres, sesja, tura, źródło).
//! 2. **Rodzina wersji** (tylko dla celu „wpis”): poprzednie i następne wersje oraz scalone
//!    duplikaty — użytkownik zapomina fakt, nie jedną jego wersję.
//! 3. **Pochodne**: wpis, którego którekolwiek źródło (`origin.derived_from`) jest usuwane, też
//!    znika (streszczenia, fakty, kopie po awansie, nowe wersje po edycji niosą treść źródła).
//!    Następna konsolidacja może odtworzyć fakt z pozostałych źródeł.
//! 4. **Przywrócenie**: wpis zastąpiony (sprzeczność, duplikat) przez wpis usuwany, sam nieusuwany,
//!    wraca do stanu aktywnego — jego treść nie pochodzi z zapominanego źródła.

use std::collections::BTreeSet;

use crate::model::EntryRef;
use crate::types::MemoryEntry;

/// Plan kaskady.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CascadePlan {
    /// Wszystkie wpisy do usunięcia.
    pub remove: BTreeSet<EntryRef>,
    /// Z tego: wpisy dodane jako rodzina wersji.
    pub versions: BTreeSet<EntryRef>,
    /// Z tego: wpisy pochodne.
    pub derived: BTreeSet<EntryRef>,
    /// Wpisy do przywrócenia (zdjęcie zastąpienia).
    pub revive: BTreeSet<EntryRef>,
}

fn same_scope(e: &MemoryEntry, id: &crate::MemoryId) -> EntryRef {
    EntryRef::new(e.scope.clone(), id.clone())
}

/// Planuje kaskadę dla nasion `seeds` na zbiorze wpisów `all` (wszystkie zakresy, które mogą
/// zawierać pochodne). `family` — dołącz rodzinę wersji nasion (cel „wpis”).
pub fn plan_cascade(all: &[MemoryEntry], seeds: &BTreeSet<EntryRef>, family: bool) -> CascadePlan {
    let mut plan = CascadePlan {
        remove: seeds.clone(),
        ..CascadePlan::default()
    };
    if family {
        loop {
            let mut added = false;
            for e in all {
                let me = e.entry_ref();
                let linked = |r: &EntryRef| plan.remove.contains(r);
                let in_family = !plan.remove.contains(&me)
                    && (e
                        .superseded
                        .as_ref()
                        .is_some_and(|s| linked(&same_scope(e, &s.by)))
                        || all.iter().any(|o| {
                            plan.remove.contains(&o.entry_ref())
                                && o.scope == e.scope
                                && (o.supersedes.as_ref() == Some(&e.id)
                                    || o.superseded.as_ref().is_some_and(|s| s.by == e.id)
                                    || e.supersedes.as_ref() == Some(&o.id))
                        }));
                if in_family {
                    plan.remove.insert(me.clone());
                    plan.versions.insert(me);
                    added = true;
                }
            }
            if !added {
                break;
            }
        }
    }
    loop {
        let mut added = false;
        for e in all {
            let me = e.entry_ref();
            if plan.remove.contains(&me) {
                continue;
            }
            if e.origin
                .derived_from
                .iter()
                .any(|p| plan.remove.contains(p))
            {
                plan.remove.insert(me.clone());
                plan.derived.insert(me);
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    for e in all {
        let me = e.entry_ref();
        if plan.remove.contains(&me) {
            continue;
        }
        if let Some(s) = &e.superseded
            && plan.remove.contains(&same_scope(e, &s.by))
        {
            plan.revive.insert(me);
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use core_bus_contract::SessionId;

    use super::*;
    use crate::model::{Derivation, Origin, SupersedeReason, Supersession};
    use crate::types::{MemoryId, MemoryScope, NewMemory};

    fn entry(scope: &MemoryScope, id: &str) -> MemoryEntry {
        let new = NewMemory::new(
            scope.clone(),
            crate::Layer::Semantic,
            id,
            crate::Provenance::User,
        );
        MemoryEntry::from_new(MemoryId(id.into()), new, DateTime::<Utc>::default(), true)
    }

    fn r(scope: &MemoryScope, id: &str) -> EntryRef {
        EntryRef::new(scope.clone(), MemoryId(id.into()))
    }

    #[test]
    fn derived_versions_and_revival() {
        let s = MemoryScope::Session(SessionId::new("A"));
        let g = MemoryScope::Global;
        let fact = entry(&s, "f");
        let mut copy = entry(&g, "g");
        copy.origin = Origin::derived(Derivation::Promoted, vec![r(&s, "f")]);
        let mut dup = entry(&g, "dup");
        dup.superseded = Some(Supersession {
            by: MemoryId("g".into()),
            reason: SupersedeReason::Duplicate,
            at: DateTime::<Utc>::default(),
        });
        let mut summary = entry(&g, "sum");
        summary.origin = Origin::derived(Derivation::Summary, vec![r(&g, "g"), r(&g, "other")]);
        let other = entry(&g, "other");
        let all = vec![fact, copy, dup, summary, other];
        let plan = plan_cascade(&all, &BTreeSet::from([r(&s, "f")]), false);
        assert!(plan.remove.contains(&r(&g, "g")) && plan.remove.contains(&r(&g, "sum")));
        assert!(!plan.remove.contains(&r(&g, "other")));
        assert_eq!(plan.revive, BTreeSet::from([r(&g, "dup")]));
        assert_eq!(plan.derived.len(), 2);
        // Cel „wpis”: duplikat i rodzina wersji znikają razem z wpisem.
        let plan = plan_cascade(&all, &BTreeSet::from([r(&g, "g")]), true);
        assert!(plan.remove.contains(&r(&g, "dup")) && plan.versions.contains(&r(&g, "dup")));
        assert!(plan.revive.is_empty());
    }

    #[test]
    fn version_chain_is_one_family() {
        let s = MemoryScope::Session(SessionId::new("A"));
        let mut v1 = entry(&s, "v1");
        let mut v2 = entry(&s, "v2");
        let mut v3 = entry(&s, "v3");
        let sup = |by: &str| Supersession {
            by: MemoryId(by.into()),
            reason: SupersedeReason::Edit,
            at: DateTime::<Utc>::default(),
        };
        v1.superseded = Some(sup("v2"));
        v2.supersedes = Some(MemoryId("v1".into()));
        v2.superseded = Some(sup("v3"));
        v3.supersedes = Some(MemoryId("v2".into()));
        let all = vec![v1, v2, v3];
        let plan = plan_cascade(&all, &BTreeSet::from([r(&s, "v2")]), true);
        assert_eq!(plan.remove.len(), 3);
        let plan = plan_cascade(&all, &BTreeSet::from([r(&s, "v3")]), false);
        assert_eq!(plan.revive, BTreeSet::from([r(&s, "v2")]));
    }
}
