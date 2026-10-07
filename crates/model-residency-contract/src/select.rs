//! Wybór ofiar eksmisji: „LRU z priorytetami" (głos > rozmowa > tło).
//!
//! Dzierżawa `l` może ustąpić żądaniu `r`, gdy `l` ma **niższy** priorytet, albo **równy** i nie jest
//! w użyciu (`in_use`). Dzięki temu żądanie o wyższym priorytecie nigdy nie czeka na niższy.

use std::collections::BTreeMap;

use crate::types::{Budget, Device, Lease, LeaseId, LeaseRequest, ModelRole, Usage};

/// Czy dzierżawa może ustąpić żądaniu.
pub fn yields_to(lease: &Lease, request: &LeaseRequest) -> bool {
    lease.priority() < request.priority || (lease.priority() == request.priority && !lease.in_use)
}

/// Klucz kolejności eksmisji: najpierw najniższy priorytet, potem nieużywane, potem najdawniej
/// używane (LRU), na końcu najstarsze id — deterministycznie.
fn eviction_key(l: &Lease) -> (crate::types::Priority, bool, u64, LeaseId) {
    (l.priority(), l.in_use, l.last_used_ms, l.id)
}

/// Suma zużycia.
pub fn total(leases: &BTreeMap<LeaseId, Lease>) -> Usage {
    leases.values().fold(Usage::default(), |acc, l| {
        let u = l.usage();
        Usage {
            vram_mb: acc.vram_mb.saturating_add(u.vram_mb),
            ram_mb: acc.ram_mb.saturating_add(u.ram_mb),
        }
    })
}

/// Czy zużycie mieści się w budżecie.
pub fn fits(used: Usage, budget: Budget) -> bool {
    used.vram_mb <= budget.vram_mb && used.ram_mb <= budget.ram_mb
}

fn plus(a: Usage, b: Usage) -> Usage {
    Usage {
        vram_mb: a.vram_mb.saturating_add(b.vram_mb),
        ram_mb: a.ram_mb.saturating_add(b.ram_mb),
    }
}

fn minus(a: Usage, b: Usage) -> Usage {
    Usage {
        vram_mb: a.vram_mb.saturating_sub(b.vram_mb),
        ram_mb: a.ram_mb.saturating_sub(b.ram_mb),
    }
}

/// Dzierżawy, które muszą zejść z GPU, gdy STT i ciężki TTS są wzajemnie wykluczające.
pub fn exclusive_conflicts<'a>(
    leases: &'a BTreeMap<LeaseId, Lease>,
    request: &LeaseRequest,
    device: Device,
    budget: Budget,
) -> Vec<&'a Lease> {
    let other = match request.role {
        ModelRole::Stt => ModelRole::Tts,
        ModelRole::Tts => ModelRole::Stt,
        _ => return Vec::new(),
    };
    if !budget.stt_tts_exclusive || device != Device::Gpu {
        return Vec::new();
    }
    leases
        .values()
        .filter(|l| l.device == Device::Gpu && l.request.role == other)
        .collect()
}

/// Wynik planowania umiejscowienia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Mieści się po eksmisji podanych dzierżaw (pusta lista = bez eksmisji).
    Fits {
        /// Ofiary „wykluczenia" STT/TTS.
        exclusive: Vec<LeaseId>,
        /// Ofiary budżetowe (w kolejności).
        preempted: Vec<LeaseId>,
    },
    /// Nie mieści się; blokują te dzierżawy (priorytet ≥ żądania).
    Blocked(Vec<LeaseId>),
}

/// Planuje umiejscowienie żądania na urządzeniu (`allow_evict = false` → tylko wolne miejsce).
pub fn plan(
    leases: &BTreeMap<LeaseId, Lease>,
    request: &LeaseRequest,
    device: Device,
    budget: Budget,
    allow_evict: bool,
) -> Plan {
    let need = request.need(device);
    let conflicts = exclusive_conflicts(leases, request, device, budget);
    let blocking: Vec<LeaseId> = conflicts
        .iter()
        .filter(|l| !yields_to(l, request))
        .map(|l| l.id)
        .collect();
    if !blocking.is_empty() {
        return Plan::Blocked(blocking);
    }
    if !allow_evict && !conflicts.is_empty() {
        return Plan::Blocked(Vec::new());
    }
    let exclusive: Vec<LeaseId> = conflicts.iter().map(|l| l.id).collect();
    let mut used = conflicts
        .iter()
        .fold(total(leases), |acc, l| minus(acc, l.usage()));
    let mut preempted = Vec::new();
    if fits(plus(used, need), budget) {
        return Plan::Fits {
            exclusive,
            preempted,
        };
    }
    if !allow_evict {
        return Plan::Blocked(Vec::new());
    }
    let mut candidates: Vec<&Lease> = leases
        .values()
        .filter(|l| !exclusive.contains(&l.id) && yields_to(l, request))
        .collect();
    candidates.sort_by_key(|l| eviction_key(l));
    for victim in candidates {
        let after = plus(used, need);
        let vram_short = after.vram_mb > budget.vram_mb;
        let ram_short = after.ram_mb > budget.ram_mb;
        let u = victim.usage();
        let helps = (vram_short && u.vram_mb > 0) || (ram_short && u.ram_mb > 0);
        if !helps {
            continue;
        }
        used = minus(used, u);
        preempted.push(victim.id);
        if fits(plus(used, need), budget) {
            return Plan::Fits {
                exclusive,
                preempted,
            };
        }
    }
    let blockers = leases
        .values()
        .filter(|l| !yields_to(l, request) && l.usage() != Usage::default())
        .map(|l| l.id)
        .collect();
    Plan::Blocked(blockers)
}

/// Ofiary przywracające budżet po jego zmniejszeniu (najpierw najniższy priorytet / LRU).
pub fn shrink_victims(leases: &BTreeMap<LeaseId, Lease>, budget: Budget) -> Vec<LeaseId> {
    let mut used = total(leases);
    let mut order: Vec<&Lease> = leases.values().collect();
    order.sort_by_key(|l| eviction_key(l));
    let mut out = Vec::new();
    for l in order {
        if fits(used, budget) {
            break;
        }
        let u = l.usage();
        let helps = (used.vram_mb > budget.vram_mb && u.vram_mb > 0)
            || (used.ram_mb > budget.ram_mb && u.ram_mb > 0);
        if helps {
            used = minus(used, u);
            out.push(l.id);
        }
    }
    out
}
