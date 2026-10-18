//! Wybór ofiar eksmisji: „LRU z priorytetami" (głos > rozmowa > tło).
//!
//! Dzierżawa `l` może ustąpić żądaniu `r`, gdy `l` ma **niższy** priorytet, albo **równy** i nie jest
//! w użyciu (`in_use`). Dzięki temu żądanie o wyższym priorytecie nigdy nie czeka na niższy.
//! Rachunki idą przez [`Pools`]: żądanie na karcie B może wyprzeć tylko dzierżawy z karty B albo
//! zajmujące RAM (gdy go brakuje) — model na karcie A nie ustępuje miejsca na karcie B.

use std::collections::BTreeMap;

use crate::gpus::GpuId;
use crate::pools::Pools;
use crate::types::{Budget, Lease, LeaseId, LeaseRequest, ModelRole, Usage};

/// Czy dzierżawa może ustąpić żądaniu.
pub fn yields_to(lease: &Lease, request: &LeaseRequest) -> bool {
    lease.priority() < request.priority || (lease.priority() == request.priority && !lease.in_use)
}

/// Klucz kolejności eksmisji: najpierw najniższy priorytet, potem nieużywane, potem najdawniej
/// używane (LRU), na końcu najstarsze id — deterministycznie.
fn eviction_key(l: &Lease) -> (crate::types::Priority, bool, u64, LeaseId) {
    (l.priority(), l.in_use, l.last_used_ms, l.id)
}

/// Suma surowa zużycia (pamięć wszystkich kart razem i RAM bez pamięci kart współdzielonych);
/// budżet sprawdzają [`Pools`].
pub fn total(leases: &BTreeMap<LeaseId, Lease>) -> Usage {
    leases
        .values()
        .fold(Usage::default(), |acc, l| plus(acc, l.usage()))
}

/// Czy zużycie mieści się w budżecie (karta główna i RAM).
pub fn fits(used: Usage, budget: Budget) -> bool {
    used.within(Usage {
        vram_mb: budget.vram_mb,
        ram_mb: budget.ram_mb,
    })
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

/// Dzierżawy, które muszą zejść z karty `on`, gdy STT i ciężki TTS są na niej wzajemnie
/// wykluczające (procesor — nigdy).
pub fn exclusive_conflicts<'a>(
    leases: &'a BTreeMap<LeaseId, Lease>,
    request: &LeaseRequest,
    on: Option<GpuId>,
    pools: &Pools,
) -> Vec<&'a Lease> {
    let other = match request.role {
        ModelRole::Stt => ModelRole::Tts,
        ModelRole::Tts => ModelRole::Stt,
        _ => return Vec::new(),
    };
    let Some(card) = on else {
        return Vec::new();
    };
    if !pools.card(card).is_some_and(|p| p.stt_tts_exclusive) {
        return Vec::new();
    }
    leases
        .values()
        .filter(|l| pools.card_of(l) == Some(card) && l.request.role == other)
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

/// Planuje umiejscowienie żądania w miejscu `on` — karta albo `None` (procesor);
/// `allow_evict = false` → tylko wolne miejsce.
pub fn plan(
    leases: &BTreeMap<LeaseId, Lease>,
    request: &LeaseRequest,
    on: Option<GpuId>,
    pools: &Pools,
    allow_evict: bool,
) -> Plan {
    let need = pools.need(request, on);
    let limit = pools.limit(on);
    let conflicts = exclusive_conflicts(leases, request, on, pools);
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
        .fold(pools.used(leases.values(), on), |acc, l| {
            minus(acc, pools.load(l, on))
        });
    let mut preempted = Vec::new();
    if plus(used, need).within(limit) {
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
        let vram_short = after.vram_mb > limit.vram_mb;
        let ram_short = after.ram_mb > limit.ram_mb;
        let u = pools.load(victim, on);
        let helps = (vram_short && u.vram_mb > 0) || (ram_short && u.ram_mb > 0);
        if !helps {
            continue;
        }
        used = minus(used, u);
        preempted.push(victim.id);
        if plus(used, need).within(limit) {
            return Plan::Fits {
                exclusive,
                preempted,
            };
        }
    }
    let blockers = leases
        .values()
        .filter(|l| !yields_to(l, request) && pools.load(l, on) != Usage::default())
        .map(|l| l.id)
        .collect();
    Plan::Blocked(blockers)
}

/// Ofiary przywracające wszystkie pule po zmniejszeniu budżetu (najpierw najniższy priorytet /
/// LRU; dzierżawa odpada tylko wtedy, gdy zwalnia przepełnioną pulę).
pub fn shrink_victims(leases: &BTreeMap<LeaseId, Lease>, pools: &Pools) -> Vec<LeaseId> {
    let mut order: Vec<&Lease> = leases.values().collect();
    order.sort_by_key(|l| eviction_key(l));
    let mut out: Vec<LeaseId> = Vec::new();
    for l in order {
        let rest = leases.values().filter(|x| !out.contains(&x.id));
        let (ram_over, cards_over) = pools.overflow(rest);
        if !ram_over && cards_over.is_empty() {
            break;
        }
        let ram = pools.load(l, None).ram_mb;
        let card = pools.card_of(l);
        let helps = (ram_over && ram > 0)
            || card.is_some_and(|c| cards_over.contains(&c) && l.usage().vram_mb > 0);
        if helps {
            out.push(l.id);
        }
    }
    out
}
