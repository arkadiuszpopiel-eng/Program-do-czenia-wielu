//! Zmiany budżetu, kart i trybu oraz bezczynność w [`LeaseTable`]: nadmiar w każdej puli
//! eksmitowany od najniższego priorytetu, wykluczenie STT/TTS na każdej karcie osobno, tryb gry
//! zwalnia wszystkie karty.

use super::{Change, LeaseTable};
use crate::gpus::GpuLayout;
use crate::select::shrink_victims;
use crate::types::{
    Budget, Device, Lease, LeaseId, Mode, ModelRole, Placement, Priority, Revocation, RevokeReason,
};

impl LeaseTable {
    /// Zmienia budżet karty głównej i RAM (zmiana sprzętu); nadmiar eksmitowany od najniższego
    /// priorytetu.
    pub fn set_budget(&mut self, budget: Budget) -> Vec<Change> {
        self.base = budget;
        self.enforce_budget()
    }

    /// Zmienia karty maszyny (zmiana sprzętu): dzierżawy na kartach, których już nie ma, są
    /// odbierane; nadmiar w pulach — jak przy zmianie budżetu.
    pub fn set_gpus(&mut self, gpus: GpuLayout) -> Vec<Change> {
        self.gpus = gpus.normalized();
        let pools = self.pools();
        let orphans: Vec<LeaseId> = self
            .leases
            .values()
            .filter(|l| l.device == Device::Gpu && l.gpu.is_some_and(|g| pools.card(g).is_none()))
            .map(|l| l.id)
            .collect();
        let mut changes = self.revoke_all(&orphans, &RevokeReason::BudgetShrunk);
        changes.extend(self.enforce_budget());
        changes
    }

    fn enforce_budget(&mut self) -> Vec<Change> {
        let victims = self.exclusive_violations();
        let mut changes = self.revoke_all(&victims, &RevokeReason::BudgetShrunk);
        let victims = shrink_victims(&self.leases, &self.pools());
        changes.extend(self.revoke_all(&victims, &RevokeReason::BudgetShrunk));
        changes
    }

    /// Po włączeniu wykluczenia STT/TTS na karcie: zostaje rola najsilniejszej dzierżawy na tej
    /// karcie (priorytet, użycie, świeżość), dzierżawy drugiej roli na tej karcie są odbierane.
    fn exclusive_violations(&self) -> Vec<LeaseId> {
        let pools = self.pools();
        let mut out = Vec::new();
        for pool in pools.cards().filter(|p| p.stt_tts_exclusive) {
            let voice: Vec<&Lease> = self
                .leases
                .values()
                .filter(|l| {
                    pools.card_of(l) == Some(pool.gpu)
                        && matches!(l.request.role, ModelRole::Stt | ModelRole::Tts)
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
            out.extend(
                voice
                    .iter()
                    .filter(|l| Some(l.request.role) != keep)
                    .map(|l| l.id),
            );
        }
        out
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

    /// Zmienia tryb: gra → dzierżawy GPU (każda karta) na CPU (gdy pozwala umiejscowienie i RAM)
    /// albo eksmisja; bateria → eksmisja tła; emulacja → egzekwowanie mniejszych pul.
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
        let pools = self.pools();
        let mut changes = Vec::new();
        for lease in on_gpu {
            let ram = pools.used(self.leases.values(), None).ram_mb;
            let after = ram
                .saturating_sub(pools.load(&lease, None).ram_mb)
                .saturating_add(lease.request.cpu_ram_mb);
            let movable = lease.request.placement != Placement::GpuOnly && after <= pools.ram_mb;
            if movable && let Some(l) = self.leases.get_mut(&lease.id) {
                l.device = Device::Cpu;
                l.gpu = None;
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
