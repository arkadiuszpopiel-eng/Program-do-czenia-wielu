//! Pule obowiązujące w danej chwili (po emulacji): RAM maszyny i pamięć każdej karty. Wszystkie
//! rachunki zarządcy — planowanie, eksmisje, niezmiennik budżetu — idą przez [`Pools`]; dzierżawa
//! na karcie liczy się tylko w puli swojej karty (i w RAM), więc model na karcie A nie blokuje
//! karty B. Miejsce dzierżawy to `Option<GpuId>`: karta albo `None` — procesor.

use std::collections::BTreeMap;

use crate::gpus::{GpuId, GpuLayout, GpuPool, GpuState};
use crate::types::{Budget, Device, Lease, LeaseId, LeaseRequest, Usage};

/// Pule obowiązujące: RAM, karta główna i pozostałe karty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pools {
    /// RAM dla modeli (MB).
    pub ram_mb: u32,
    /// Karta główna (pula z [`Budget`]).
    pub primary: GpuPool,
    /// Pozostałe karty.
    pub others: Vec<GpuPool>,
}

impl Pools {
    /// Pule z budżetu karty głównej i RAM (już po emulacji) oraz kart maszyny; emulacja
    /// słabszej maszyny ogranicza też pozostałe karty ([`GpuPool::limited`]).
    pub fn new(budget: Budget, layout: &GpuLayout, emulated: Option<Budget>) -> Self {
        let others = layout
            .others
            .iter()
            .map(|p| emulated.map_or(*p, |e| p.limited(e)))
            .collect();
        Self {
            ram_mb: budget.ram_mb,
            primary: GpuPool::primary(layout.primary, budget),
            others,
        }
    }

    /// Wszystkie karty, główna pierwsza.
    pub fn cards(&self) -> impl Iterator<Item = &GpuPool> {
        std::iter::once(&self.primary).chain(self.others.iter())
    }

    /// Pula karty (`None` — karta nieznana).
    pub fn card(&self, gpu: GpuId) -> Option<&GpuPool> {
        self.cards().find(|p| p.gpu == gpu)
    }

    /// Karta docelowa żądania: wskazana, jeśli ma pulę; bez wskazania albo nieznana — główna.
    pub fn resolve(&self, wanted: Option<GpuId>) -> GpuId {
        wanted
            .filter(|g| self.card(*g).is_some())
            .unwrap_or(self.primary.gpu)
    }

    /// Karta dzierżawy (`None` — dzierżawa na procesorze).
    pub fn card_of(&self, lease: &Lease) -> Option<GpuId> {
        match lease.device {
            Device::Cpu => None,
            Device::Gpu => Some(self.resolve(lease.gpu)),
        }
    }

    fn shared(&self, card: Option<GpuId>) -> bool {
        card.and_then(|c| self.card(c)).is_some_and(|p| p.shared)
    }

    /// Zużycie dzierżawy widziane z miejsca `on`: pamięć tej samej karty (inna karta i procesor —
    /// 0) oraz RAM, do którego wlicza się pamięć karty współdzielonej.
    pub fn load(&self, lease: &Lease, on: Option<GpuId>) -> Usage {
        let usage = lease.usage();
        let card = self.card_of(lease);
        let shared = if self.shared(card) { usage.vram_mb } else { 0 };
        Usage {
            vram_mb: if card.is_some() && card == on {
                usage.vram_mb
            } else {
                0
            },
            ram_mb: usage.ram_mb.saturating_add(shared),
        }
    }

    /// Zapotrzebowanie żądania w miejscu `on` (karta albo `None` — procesor).
    pub fn need(&self, request: &LeaseRequest, on: Option<GpuId>) -> Usage {
        let Some(card) = on else {
            return request.need(Device::Cpu);
        };
        let usage = request.need(Device::Gpu);
        let shared = if self.shared(Some(card)) {
            usage.vram_mb
        } else {
            0
        };
        Usage {
            vram_mb: usage.vram_mb,
            ram_mb: usage.ram_mb.saturating_add(shared),
        }
    }

    /// Limit miejsca `on`: pamięć karty (procesor — 0) i RAM.
    pub fn limit(&self, on: Option<GpuId>) -> Usage {
        Usage {
            vram_mb: on.and_then(|c| self.card(c)).map_or(0, |p| p.vram_mb),
            ram_mb: self.ram_mb,
        }
    }

    /// Zajęte widziane z miejsca `on`.
    pub fn used<'a>(
        &self,
        leases: impl IntoIterator<Item = &'a Lease>,
        on: Option<GpuId>,
    ) -> Usage {
        leases.into_iter().fold(Usage::default(), |acc, l| {
            let u = self.load(l, on);
            Usage {
                vram_mb: acc.vram_mb.saturating_add(u.vram_mb),
                ram_mb: acc.ram_mb.saturating_add(u.ram_mb),
            }
        })
    }

    /// Przepełnione pule: czy RAM i które karty.
    pub fn overflow<'a>(
        &self,
        leases: impl IntoIterator<Item = &'a Lease> + Clone,
    ) -> (bool, Vec<GpuId>) {
        let ram = self.used(leases.clone(), None).ram_mb > self.ram_mb;
        let cards = self
            .cards()
            .filter(|p| self.used(leases.clone(), Some(p.gpu)).vram_mb > p.vram_mb)
            .map(|p| p.gpu)
            .collect();
        (ram, cards)
    }

    /// Czy wszystkie pule mieszczą dzierżawy.
    pub fn fit(&self, leases: &BTreeMap<LeaseId, Lease>) -> bool {
        let (ram, cards) = self.overflow(leases.values());
        !ram && cards.is_empty()
    }

    /// Stan kart dla migawki (główna pierwsza).
    pub fn states(&self, leases: &BTreeMap<LeaseId, Lease>) -> Vec<GpuState> {
        self.cards()
            .map(|p| GpuState {
                pool: *p,
                primary: p.gpu == self.primary.gpu,
                used_mb: self.used(leases.values(), Some(p.gpu)).vram_mb,
            })
            .collect()
    }
}
