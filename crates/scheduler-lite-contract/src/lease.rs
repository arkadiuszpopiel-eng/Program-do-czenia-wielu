//! Dzierżawa RAII: zwolnienie przy `drop`, sygnał wywłaszczenia do sprawdzania w punktach atomowych.

use std::sync::Weak;

use tokio::sync::watch;

use crate::types::{Holder, LeaseId, LeaseInfo, LeaseSignal, RequestId, Resource, SchedError};

/// Operacje sterownika wywoływane przez dzierżawę i porzucone żądania.
pub trait LeaseControl: Send + Sync {
    /// Zwalnia dzierżawę (nieznana → nic).
    fn release(&self, lease: LeaseId);
    /// Przekazuje dzierżawę bez luki.
    fn handoff(&self, lease: LeaseId, to: Holder) -> Result<(), SchedError>;
    /// Porzuca czekające żądanie (wołająca przestała czekać).
    fn cancel(&self, request: RequestId);
}

/// Przyznany zasób. Zwalniany automatycznie przy `drop` (także przy panice wątku posiadaczki).
pub struct Lease {
    info: LeaseInfo,
    control: Weak<dyn LeaseControl>,
    signal: watch::Receiver<LeaseSignal>,
    armed: bool,
}

impl std::fmt::Debug for Lease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lease")
            .field("info", &self.info)
            .field("signal", &*self.signal.borrow())
            .field("armed", &self.armed)
            .finish()
    }
}

impl Lease {
    /// Tworzy dzierżawę (wywołuje sterownik schedulera).
    pub fn new(
        info: LeaseInfo,
        control: Weak<dyn LeaseControl>,
        signal: watch::Receiver<LeaseSignal>,
    ) -> Self {
        Self {
            info,
            control,
            signal,
            armed: true,
        }
    }

    /// Opis dzierżawy.
    pub fn info(&self) -> &LeaseInfo {
        &self.info
    }

    /// Identyfikator.
    pub fn id(&self) -> LeaseId {
        self.info.id
    }

    /// Zasób.
    pub fn resource(&self) -> &Resource {
        &self.info.resource
    }

    /// Posiadaczka.
    pub fn holder(&self) -> &Holder {
        &self.info.holder
    }

    /// Bieżący sygnał — sprawdzany przez posiadaczkę w punktach atomowych.
    pub fn signal(&self) -> LeaseSignal {
        self.signal.borrow().clone()
    }

    /// Czy poproszono o zwolnienie (wywłaszczenie w punkcie atomowym).
    pub fn preempt_requested(&self) -> bool {
        matches!(*self.signal.borrow(), LeaseSignal::PreemptRequested { .. })
    }

    /// Czy dzierżawa została odebrana (kill-switch, przekazanie) — zasób już nie jest jej.
    pub fn is_revoked(&self) -> bool {
        matches!(*self.signal.borrow(), LeaseSignal::Revoked { .. })
    }

    /// Czeka na zmianę sygnału (np. `select!` z pracą posiadaczki) i zwraca nowy sygnał.
    pub async fn changed(&mut self) -> LeaseSignal {
        if self.signal.changed().await.is_err() {
            // Sterownik zniknął: traktuj jak odebranie.
            return LeaseSignal::Revoked {
                reason: crate::types::PreemptReason::KillSwitch,
            };
        }
        self.signal.borrow_and_update().clone()
    }

    /// Jawne zwolnienie (to samo co `drop`).
    pub fn release(self) {
        drop(self);
    }

    /// Przekazuje zasób innej posiadaczce bez luki („Przekazuję Delcie…”). Po sukcesie ta
    /// dzierżawa jest nieaktywna (drop nic nie robi); przy błędzie pozostaje ważna.
    pub fn handoff(&mut self, to: Holder) -> Result<(), SchedError> {
        let control = self.control.upgrade().ok_or(SchedError::UnknownLease)?;
        control.handoff(self.info.id, to)?;
        self.armed = false;
        Ok(())
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if self.armed
            && let Some(control) = self.control.upgrade()
        {
            control.release(self.info.id);
        }
    }
}
