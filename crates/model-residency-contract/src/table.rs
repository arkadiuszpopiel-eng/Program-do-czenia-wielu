//! Tablica dzierżaw — czysta maszyna stanów wspólna dla `-impl` i `-fake` (bez zegara i I/O;
//! czas podaje wywołujący). Niezmiennik: po każdej operacji suma dzierżaw ≤ budżet obowiązujący.

use std::collections::BTreeMap;

use crate::select::{Plan, fits, plan, shrink_victims, total};
use crate::types::{
    Budget, Device, Grant, Lease, LeaseId, LeaseRequest, Mode, Placement, Priority, ResidencyError,
    ResidencyState, Revocation, RevokeReason, Usage,
};

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
    mode: Mode,
    leases: BTreeMap<LeaseId, Lease>,
    next_id: u64,
}

impl LeaseTable {
    /// Pusta tablica z budżetem maszyny.
    pub fn new(budget: Budget) -> Self {
        Self {
            base: budget,
            mode: Mode::normal(),
            leases: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Budżet obowiązujący (po emulacji).
    pub fn budget(&self) -> Budget {
        match self.mode.emulated {
            Some(e) => self.base.min(e),
            None => self.base,
        }
    }

    /// Tryb.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Zajęte zasoby.
    pub fn used(&self) -> Usage {
        total(&self.leases)
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
        }
    }

    /// Czy niezmiennik budżetu jest spełniony.
    pub fn within_budget(&self) -> bool {
        fits(self.used(), self.budget())
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
    /// bez wypierania, inaczej CPU ([`Placement::GpuIfFree`]).
    pub fn acquire(
        &mut self,
        request: &LeaseRequest,
        now_ms: u64,
    ) -> Result<Grant, ResidencyError> {
        Self::validate(request)?;
        let budget = self.budget();
        let devices = self.devices(request)?;
        let feasible: Vec<Device> = devices
            .into_iter()
            .filter(|d| fits(request.need(*d), budget))
            .collect();
        if feasible.is_empty() {
            return Err(ResidencyError::TooLarge {
                model: request.model.clone(),
                vram_mb: budget.vram_mb,
                ram_mb: budget.ram_mb,
            });
        }
        let mut blockers = Vec::new();
        for (device, allow_evict, preempt) in Self::attempts(&feasible, request.placement) {
            match plan(&self.leases, request, device, budget, allow_evict) {
                Plan::Fits {
                    exclusive,
                    preempted,
                } if preempt || preempted.is_empty() => {
                    return Ok(self.grant(request, device, &exclusive, &preempted, now_ms));
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
        device: Device,
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

    /// Zmienia budżet maszyny (zmiana sprzętu); nadmiar eksmitowany od najniższego priorytetu.
    pub fn set_budget(&mut self, budget: Budget) -> Vec<Change> {
        self.base = budget;
        self.enforce_budget()
    }

    fn enforce_budget(&mut self) -> Vec<Change> {
        let mut victims = self.exclusive_violations();
        let mut changes = self.revoke_all(&victims, &RevokeReason::BudgetShrunk);
        victims = shrink_victims(&self.leases, self.budget());
        changes.extend(self.revoke_all(&victims, &RevokeReason::BudgetShrunk));
        changes
    }

    /// Po włączeniu wykluczenia STT/TTS: zostaje rola najsilniejszej dzierżawy na GPU
    /// (priorytet, użycie, świeżość), dzierżawy GPU drugiej roli są odbierane.
    fn exclusive_violations(&self) -> Vec<LeaseId> {
        use crate::types::ModelRole;
        if !self.budget().stt_tts_exclusive {
            return Vec::new();
        }
        let voice: Vec<&Lease> = self
            .leases
            .values()
            .filter(|l| {
                l.device == Device::Gpu && matches!(l.request.role, ModelRole::Stt | ModelRole::Tts)
            })
            .collect();
        let keep = voice
            .iter()
            .max_by_key(|l| {
                (
                    l.priority(),
                    l.in_use,
                    l.last_used_ms,
                    std::cmp::Reverse(l.id),
                )
            })
            .map(|l| l.request.role);
        voice
            .iter()
            .filter(|l| Some(l.request.role) != keep)
            .map(|l| l.id)
            .collect()
    }

    fn revoke_all(&mut self, ids: &[LeaseId], reason: &RevokeReason) -> Vec<Change> {
        ids.iter()
            .filter_map(|id| self.leases.remove(id))
            .map(|lease| {
                Change::Revoked(Revocation {
                    lease,
                    reason: reason.clone(),
                })
            })
            .collect()
    }

    /// Zmienia tryb: gra → dzierżawy GPU na CPU (gdy pozwala umiejscowienie i RAM) albo eksmisja;
    /// bateria → eksmisja tła; emulacja → egzekwowanie mniejszego budżetu.
    pub fn set_mode(&mut self, mode: Mode) -> Vec<Change> {
        self.mode = mode;
        let mut changes = Vec::new();
        if mode.battery {
            let background: Vec<LeaseId> = self
                .leases
                .values()
                .filter(|l| l.priority() == Priority::Background)
                .map(|l| l.id)
                .collect();
            changes.extend(self.revoke_all(&background, &RevokeReason::Battery));
        }
        if mode.gaming {
            changes.extend(self.leave_gpu());
        }
        changes.extend(self.enforce_budget());
        changes
    }

    fn leave_gpu(&mut self) -> Vec<Change> {
        let mut on_gpu: Vec<Lease> = self
            .leases
            .values()
            .filter(|l| l.device == Device::Gpu)
            .cloned()
            .collect();
        // Wyższy priorytet dostaje RAM pierwszy.
        on_gpu.sort_by_key(|l| (std::cmp::Reverse(l.priority()), l.id));
        let budget = self.budget();
        let mut changes = Vec::new();
        for lease in on_gpu {
            let mut used = self.used();
            used.vram_mb = used.vram_mb.saturating_sub(lease.request.vram_mb);
            used.ram_mb = used.ram_mb.saturating_sub(lease.request.ram_mb);
            let after = used.ram_mb.saturating_add(lease.request.cpu_ram_mb);
            let movable = lease.request.placement != Placement::GpuOnly && after <= budget.ram_mb;
            if movable && let Some(l) = self.leases.get_mut(&lease.id) {
                l.device = Device::Cpu;
                changes.push(Change::Moved(l.clone()));
            } else {
                changes.extend(self.revoke_all(&[lease.id], &RevokeReason::Gaming));
            }
        }
        changes
    }

    /// Eksmituje dzierżawy bezczynne dłużej niż ich `idle_unload_ms` (tylko nieużywane).
    pub fn reap_idle(&mut self, now_ms: u64) -> Vec<Revocation> {
        let idle: Vec<LeaseId> = self
            .leases
            .values()
            .filter(|l| {
                !l.in_use
                    && l.request.idle_unload_ms > 0
                    && now_ms.saturating_sub(l.last_used_ms) >= l.request.idle_unload_ms
            })
            .map(|l| l.id)
            .collect();
        self.revoke_all(&idle, &RevokeReason::Idle)
            .into_iter()
            .filter_map(|c| match c {
                Change::Revoked(r) => Some(r),
                Change::Moved(_) => None,
            })
            .collect()
    }
}
