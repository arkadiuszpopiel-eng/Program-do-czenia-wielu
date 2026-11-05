//! Bezpiecznik sędziego: po serii odmów decyzja wraca do właściciela (plan 50 §4.1, §4.3).
//!
//! Czysta maszyna stanów — Broker trzyma po jednej na sesję i zapisuje zdarzenia w Audycie.

use crate::JudgeVerdict;

/// Progi bezpiecznika.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuseLimits {
    /// Odmowy z rzędu, po których bezpiecznik się otwiera (≥ 1).
    pub consecutive: u32,
    /// Odmowy w jednym zadaniu, po których bezpiecznik się otwiera (≥ 1).
    pub per_task: u32,
}

impl Default for FuseLimits {
    fn default() -> Self {
        Self {
            consecutive: 3,
            per_task: 20,
        }
    }
}

/// Zmiana stanu bezpiecznika (do Audytu).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuseEvent {
    /// Bez zmiany stanu.
    None,
    /// Bezpiecznik właśnie się otworzył — prośby idą prosto do właściciela.
    Tripped,
    /// Bezpiecznik zamknięty zgodą właściciela.
    Reset,
}

/// Stan bezpiecznika jednej sesji. Start zamknięty (sędzia działa).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeFuse {
    limits: FuseLimits,
    consecutive: u32,
    task_denials: u32,
    open: bool,
}

impl JudgeFuse {
    /// Nowy, zamknięty bezpiecznik. Progi poniżej 1 traktowane jak 1 (zachowawczo).
    pub fn new(limits: FuseLimits) -> Self {
        Self {
            limits: FuseLimits {
                consecutive: limits.consecutive.max(1),
                per_task: limits.per_task.max(1),
            },
            consecutive: 0,
            task_denials: 0,
            open: false,
        }
    }

    /// Czy bezpiecznik jest otwarty (sędzia pominięty, decyduje właściciel).
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Odmowy z rzędu.
    pub fn consecutive(&self) -> u32 {
        self.consecutive
    }

    /// Odmowy w bieżącym zadaniu.
    pub fn task_denials(&self) -> u32 {
        self.task_denials
    }

    /// Zapisuje werdykt sędziego. `Allow` zeruje serię, `Escalate` jej nie zmienia, `Deny` liczy.
    pub fn record(&mut self, verdict: &JudgeVerdict) -> FuseEvent {
        match verdict {
            JudgeVerdict::Allow { .. } => {
                self.consecutive = 0;
                FuseEvent::None
            }
            JudgeVerdict::Escalate { .. } => FuseEvent::None,
            JudgeVerdict::Deny { .. } => {
                self.consecutive = self.consecutive.saturating_add(1);
                self.task_denials = self.task_denials.saturating_add(1);
                let over = self.consecutive >= self.limits.consecutive
                    || self.task_denials >= self.limits.per_task;
                if over && !self.open {
                    self.open = true;
                    FuseEvent::Tripped
                } else {
                    FuseEvent::None
                }
            }
        }
    }

    /// Decyzja właściciela na karcie. Zgoda przy otwartym bezpieczniku zamyka go i zeruje liczniki;
    /// odmowa niczego nie luzuje.
    pub fn owner_decided(&mut self, approved: bool) -> FuseEvent {
        if approved && self.open {
            self.open = false;
            self.consecutive = 0;
            self.task_denials = 0;
            FuseEvent::Reset
        } else {
            FuseEvent::None
        }
    }

    /// Nowe zadanie (nowy przebieg główny): zeruje licznik zadania. Otwarty bezpiecznik zostaje otwarty —
    /// zamyka go tylko zgoda właściciela.
    pub fn new_task(&mut self) {
        self.task_denials = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DenyCategory;

    fn deny() -> JudgeVerdict {
        JudgeVerdict::Deny {
            category: DenyCategory::Other,
            reason: "x".into(),
        }
    }

    fn allow() -> JudgeVerdict {
        JudgeVerdict::Allow { reason: "x".into() }
    }

    #[test]
    fn three_in_a_row_trips_once() {
        let mut f = JudgeFuse::new(FuseLimits::default());
        assert_eq!(f.record(&deny()), FuseEvent::None);
        assert_eq!(f.record(&deny()), FuseEvent::None);
        assert_eq!(f.record(&deny()), FuseEvent::Tripped);
        assert!(f.is_open());
        assert_eq!(f.record(&deny()), FuseEvent::None);
    }

    #[test]
    fn allow_breaks_the_series_escalate_does_not() {
        let mut f = JudgeFuse::new(FuseLimits::default());
        f.record(&deny());
        f.record(&deny());
        f.record(&allow());
        f.record(&deny());
        f.record(&deny());
        assert!(!f.is_open());
        f.record(&JudgeVerdict::escalate("?"));
        assert_eq!(f.record(&deny()), FuseEvent::Tripped);
    }

    #[test]
    fn twenty_per_task_trips() {
        let mut f = JudgeFuse::new(FuseLimits::default());
        for _ in 0..19 {
            f.record(&deny());
            f.record(&allow());
        }
        assert!(!f.is_open());
        assert_eq!(f.record(&deny()), FuseEvent::Tripped);
    }

    #[test]
    fn new_task_resets_task_counter_only() {
        let mut f = JudgeFuse::new(FuseLimits::default());
        for _ in 0..19 {
            f.record(&deny());
            f.record(&allow());
        }
        f.new_task();
        assert_eq!(f.task_denials(), 0);
        assert_eq!(f.record(&deny()), FuseEvent::None);
    }

    #[test]
    fn new_task_keeps_open_fuse_open() {
        let mut f = JudgeFuse::new(FuseLimits::default());
        for _ in 0..3 {
            f.record(&deny());
        }
        f.new_task();
        assert!(f.is_open());
    }

    #[test]
    fn owner_approval_resets_denial_keeps_open() {
        let mut f = JudgeFuse::new(FuseLimits::default());
        for _ in 0..3 {
            f.record(&deny());
        }
        assert_eq!(f.owner_decided(false), FuseEvent::None);
        assert!(f.is_open());
        assert_eq!(f.owner_decided(true), FuseEvent::Reset);
        assert!(!f.is_open());
        assert_eq!(f.consecutive(), 0);
        assert_eq!(f.owner_decided(true), FuseEvent::None);
    }

    #[test]
    fn zero_limits_are_clamped() {
        let mut f = JudgeFuse::new(FuseLimits {
            consecutive: 0,
            per_task: 0,
        });
        assert_eq!(f.record(&deny()), FuseEvent::Tripped);
    }
}
