//! Tablica dzierżaw — czysta maszyna stanów wspólna dla `-impl` i `-fake` (bez zegara i I/O;
//! czas podaje wywołujący). Niezmiennik: po każdej operacji każda pula (RAM, karta główna, każda
//! pozostała karta) mieści swoje dzierżawy.

use std::collections::BTreeMap;

use crate::gpus::{GpuId, GpuLayout};
use crate::pools::Pools;
use crate::select::{Plan, plan};
use crate::types::{
    Budget, Device, Grant, Lease, LeaseId, LeaseRequest, Mode, Placement, Priority, ResidencyError,
    ResidencyState, Revocation, RevokeReason, Usage,
};

#[path = "table_mode.rs"]
mod mode;

/// Zmiana wywołana trybem/budżetem/bezczynnością.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Dzierżawa odebrana.
    Revoked(Revocation),
    /// Dzierżawa przeniesiona z GPU na CPU (właściciel przeładowuje model na CPU).
    Moved(Lease),
}

/// Tablica dzierżaw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseTable {
    base: Budget,
    gpus: GpuLayout,
    mode: Mode,
    leases: BTreeMap<LeaseId, Lease>,
    next_id: u64,
}

impl LeaseTable {
    /// Pusta tablica z budżetem maszyny z jedną kartą (wszystkie dzierżawy GPU w budżecie karty
    /// głównej — [`GpuLayout::default`]).
    pub fn new(budget: Budget) -> Self {
        Self::with_gpus(budget, GpuLayout::default())
    }

    /// Pusta tablica z budżetem karty głównej i RAM oraz pulami pozostałych kart.
    pub fn with_gpus(budget: Budget, gpus: GpuLayout) -> Self {
        Self {
            base: budget,
            gpus: gpus.normalized(),
            mode: Mode::normal(),
            leases: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Budżet obowiązujący karty głównej i RAM (po emulacji).
    pub fn budget(&self) -> Budget {
        match self.mode.emulated {
            Some(e) => self.base.min(e),
            None => self.base,
        }
    }

    /// Budżet karty głównej i RAM ze sprzętu (bez emulacji) — do porównania przy zmianie sprzętu.
    pub fn base_budget(&self) -> Budget {
        self.base
    }

    /// Karty maszyny (bez emulacji).
    pub fn gpus(&self) -> &GpuLayout {
        &self.gpus
    }

    /// Pule obowiązujące (po emulacji).
    pub fn pools(&self) -> Pools {
        Pools::new(self.budget(), &self.gpus, self.mode.emulated)
    }

    /// Tryb.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Zajęte: pamięć karty głównej i RAM (z pamięcią kart współdzielonych).
    pub fn used(&self) -> Usage {
        let pools = self.pools();
        pools.used(self.leases.values(), Some(pools.primary.gpu))
    }

    /// Pamięć karty zajęta przez dzierżawy (MB); karta nieznana — 0.
    pub fn used_on(&self, gpu: GpuId) -> u32 {
        let pools = self.pools();
        if pools.card(gpu).is_none() {
            return 0;
        }
        pools.used(self.leases.values(), Some(gpu)).vram_mb
    }

    /// Dzierżawa (jeśli aktywna).
    pub fn lease(&self, id: LeaseId) -> Option<&Lease> {
        self.leases.get(&id)
    }

    /// Migawka.
    pub fn snapshot(&self) -> ResidencyState {
        ResidencyState {
            budget: self.budget(),
            used: self.used(),
            mode: self.mode,
            leases: self.leases.values().cloned().collect(),
            gpus: self.pools().states(&self.leases),
        }
    }

    /// Czy niezmiennik budżetu jest spełniony (wszystkie pule).
    pub fn within_budget(&self) -> bool {
        self.pools().fit(&self.leases)
    }

    fn validate(request: &LeaseRequest) -> Result<(), ResidencyError> {
        let bad = |m: &str| Err(ResidencyError::Invalid(m.to_owned()));
        if request.owner.trim().is_empty() {
            return bad("pusty właściciel");
        }
        if request.model.trim().is_empty() {
            return bad("pusty model");
        }
        Ok(())
    }

    fn devices(&self, request: &LeaseRequest) -> Result<Vec<Device>, ResidencyError> {
        if self.mode.battery && request.priority == Priority::Background {
            return Err(ResidencyError::Battery);
        }
        Ok(match (request.placement, self.mode.gaming) {
            (Placement::GpuOnly, true) => return Err(ResidencyError::Gaming),
            (Placement::GpuOnly, false) => vec![Device::Gpu],
            (Placement::GpuPreferred | Placement::GpuIfFree, false) => {
                vec![Device::Gpu, Device::Cpu]
            }
            (Placement::GpuPreferred | Placement::GpuIfFree, true) | (Placement::CpuOnly, _) => {
                vec![Device::Cpu]
            }
        })
    }

    /// Kolejne próby umiejscowienia: (urządzenie, eksmisja, wypieranie z budżetu). Zwykle każde
    /// urządzenie najpierw bez eksmisji, potem z eksmisją; `GpuIfFree`: GPU bez wypierania
    /// (wymiana STT ↔ TTS dozwolona) → CPU → GPU z wypieraniem (ostatnia możliwość).
    fn attempts(feasible: &[Device], placement: Placement) -> Vec<(Device, bool, bool)> {
        let both = |d: &Device| [(*d, false, true), (*d, true, true)];
        let gpu = feasible.contains(&Device::Gpu);
        if placement != Placement::GpuIfFree || !gpu {
            return feasible.iter().flat_map(both).collect();
        }
        let mut out = vec![(Device::Gpu, false, false), (Device::Gpu, true, false)];
        out.extend(
            feasible
                .iter()
                .filter(|d| **d == Device::Cpu)
                .flat_map(both),
        );
        out.push((Device::Gpu, true, true));
        out
    }

    /// Przydziela dzierżawę: najpierw wolne miejsce, potem eksmisja ustępujących (niższy priorytet
    /// albo równy i nieużywany, LRU); GPU przed CPU dla `GpuPreferred`; `GpuIfFree` — GPU tylko
    /// bez wypierania, inaczej CPU ([`Placement::GpuIfFree`]). Na GPU — karta z żądania
    /// ([`LeaseRequest::gpu`]; brak albo nieznana — główna) i tylko jej pula; bez przenoszenia na
    /// inną kartę.
    pub fn acquire(
        &mut self,
        request: &LeaseRequest,
        now_ms: u64,
    ) -> Result<Grant, ResidencyError> {
        Self::validate(request)?;
        let pools = self.pools();
        let card = pools.resolve(request.gpu);
        let at = |d: Device| (d == Device::Gpu).then_some(card);
        let devices = self.devices(request)?;
        let feasible: Vec<Device> = devices
            .into_iter()
            .filter(|d| pools.need(request, at(*d)).within(pools.limit(at(*d))))
            .collect();
        if feasible.is_empty() {
            return Err(ResidencyError::TooLarge {
                model: request.model.clone(),
                vram_mb: pools.limit(Some(card)).vram_mb,
                ram_mb: pools.ram_mb,
            });
        }
        let mut blockers = Vec::new();
        for (device, allow_evict, preempt) in Self::attempts(&feasible, request.placement) {
            match plan(&self.leases, request, at(device), &pools, allow_evict) {
                Plan::Fits {
                    exclusive,
                    preempted,
                } if preempt || preempted.is_empty() => {
                    let place = (device, at(device));
                    return Ok(self.grant(request, place, &exclusive, &preempted, now_ms));
                }
                Plan::Fits { .. } => {}
                Plan::Blocked(b) => blockers.extend(b),
            }
        }
        blockers.sort_unstable();
        blockers.dedup();
        Err(ResidencyError::Wait { blockers })
    }

    fn grant(
        &mut self,
        request: &LeaseRequest,
        (device, gpu): (Device, Option<GpuId>),
        exclusive: &[LeaseId],
        preempted: &[LeaseId],
        now_ms: u64,
    ) -> Grant {
        let id = LeaseId(self.next_id);
        self.next_id += 1;
        let mut evicted = Vec::new();
        for (ids, reason) in [
            (exclusive, RevokeReason::Exclusive { by: id }),
            (preempted, RevokeReason::Preempted { by: id }),
        ] {
            for victim in ids {
                if let Some(lease) = self.leases.remove(victim) {
                    evicted.push(Revocation {
                        lease,
                        reason: reason.clone(),
                    });
                }
            }
        }
        let lease = Lease {
            id,
            request: request.clone(),
            device,
            granted_at_ms: now_ms,
            last_used_ms: now_ms,
            in_use: false,
            gpu,
        };
        self.leases.insert(id, lease.clone());
        Grant { lease, evicted }
    }

    /// Zwalnia dzierżawę.
    pub fn release(&mut self, id: LeaseId) -> Result<Lease, ResidencyError> {
        self.leases
            .remove(&id)
            .ok_or(ResidencyError::UnknownLease(id))
    }

    /// Odświeża licznik bezczynności.
    pub fn touch(&mut self, id: LeaseId, now_ms: u64) -> Result<(), ResidencyError> {
        let lease = self
            .leases
            .get_mut(&id)
            .ok_or(ResidencyError::UnknownLease(id))?;
        lease.last_used_ms = lease.last_used_ms.max(now_ms);
        Ok(())
    }

    /// Oznacza użycie (np. tura głosu) — równy priorytet nie wyprze dzierżawy w użyciu.
    pub fn set_in_use(
        &mut self,
        id: LeaseId,
        in_use: bool,
        now_ms: u64,
    ) -> Result<(), ResidencyError> {
        self.touch(id, now_ms)?;
        if let Some(lease) = self.leases.get_mut(&id) {
            lease.in_use = in_use;
        }
        Ok(())
    }
}
