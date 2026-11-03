//! Profil Dziecko w Sędzim (plan 50 §4.4, Z.6): kłódka jako sufit w tabeli autonomii — obowiązuje
//! każde działanie, niezależnie od tego, co pokazuje interfejs. PIN rodzica sprawdza tylko Sędzia;
//! w Audycie zostaje rodzaj zmiany i wynik, nigdy PIN.

use std::path::PathBuf;
use std::sync::MutexGuard;

use safety_broker_contract::{
    BrokerError, ChildCeiling, ChildLockChange, ChildLockView, EVENT_CHILD_LOCK, OPEN_UNTIL_CLOSE,
    Pin,
};
use serde_json::json;

use crate::child_store::{Check, ChildStore, PinParams};
use crate::engine::BrokerEngine;

/// Plik profilu Dziecko w katalogu danych Sędziego (obok zapamiętanego poziomu).
pub const CHILD_LOCK_FILE: &str = "child-lock.json";

fn refused(text: &str) -> BrokerError {
    BrokerError::InvalidRequest(text.to_owned())
}

fn io(e: &std::io::Error) -> BrokerError {
    BrokerError::InvalidRequest(format!("zapis profilu Dziecko nie powiódł się: {e}"))
}

impl BrokerEngine {
    /// Profil Dziecko z pliku `path` (prywatny katalog Sędziego); włączony albo uszkodzony zapis
    /// zamyka kłódkę od startu.
    #[must_use]
    pub fn with_child_lock(self, path: PathBuf, params: PinParams) -> Self {
        let store = ChildStore::load(Some(path), params);
        let locks = store.locks();
        *self.child_store() = store;
        if locks {
            self.lock().autonomy.set_child(Some(ChildCeiling::locked()));
        }
        self
    }

    pub(crate) fn child_store(&self) -> MutexGuard<'_, ChildStore> {
        self.child.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `Broker::child_lock`.
    pub(crate) fn child_view(&self) -> ChildLockView {
        let now = self.now();
        let store = self.child_store();
        let ceiling = self.lock().autonomy.child();
        ChildLockView {
            pin_set: store.pin_set(),
            enabled: store.locks(),
            open_until_ms: ceiling
                .filter(|c| !c.is_locked(now))
                .and_then(|c| c.open_until_ms),
            blocked_until_ms: (now < store.record.blocked_until_ms)
                .then_some(store.record.blocked_until_ms),
            attempts_left: store.attempts_left(),
        }
    }

    /// `Broker::change_child_lock`. Kolejność blokad: profil, potem stan (jak w `child_view`).
    pub(crate) fn change_child_sync(
        &self,
        change: ChildLockChange,
    ) -> Result<ChildLockView, BrokerError> {
        let now = self.now();
        let mut store = self.child_store();
        let op = match &change {
            ChildLockChange::SetPin { .. } => "set_pin",
            ChildLockChange::Enable { on: true, .. } => "enable",
            ChildLockChange::Enable { on: false, .. } => "disable",
            ChildLockChange::Open { .. } => "open",
            ChildLockChange::Close => "close",
            ChildLockChange::Verify { .. } => "verify",
        };
        let result = self.apply_child(&mut store, change, now);
        // Audyt każdej próby (bez PIN-u); błąd Audytu przy udanej zmianie = błąd żądania.
        let payload = json!({ "op": op, "ok": result.is_ok() });
        let audited = self.audit(EVENT_CHILD_LOCK, None, payload);
        drop(store);
        result?;
        audited?;
        Ok(self.child_view())
    }

    fn apply_child(
        &self,
        store: &mut ChildStore,
        change: ChildLockChange,
        now: u64,
    ) -> Result<(), BrokerError> {
        match change {
            ChildLockChange::Verify { pin } => Self::require(store, &pin, now),
            ChildLockChange::Close => {
                if store.locks() {
                    self.set_ceiling(Some(ChildCeiling::locked()));
                }
                Ok(())
            }
            ChildLockChange::SetPin { old, new } => {
                if !new.is_valid() {
                    return Err(refused("PIN ma od 4 do 8 cyfr"));
                }
                if store.corrupt {
                    return Err(refused(
                        "zapis profilu Dziecko jest uszkodzony — reset przez administratora komputera",
                    ));
                }
                if store.pin_set() {
                    let old = old.ok_or_else(|| refused("zmiana PIN-u wymaga starego PIN-u"))?;
                    Self::require(store, &old, now)?;
                }
                store.set_pin(&new).map_err(|e| io(&e))
            }
            ChildLockChange::Enable { pin, on } => {
                Self::require(store, &pin, now)?;
                let before = store.record.enabled;
                store.record.enabled = on;
                if let Err(e) = store.save() {
                    // Włączenie obowiązuje i bez zapisu (bezpieczny kierunek); wyłączenie — nie.
                    store.record.enabled = before || on;
                    if store.record.enabled {
                        self.set_ceiling(Some(ChildCeiling::locked()));
                    }
                    return Err(io(&e));
                }
                self.set_ceiling(on.then(ChildCeiling::locked));
                Ok(())
            }
            ChildLockChange::Open { pin, for_ms } => {
                if !store.locks() {
                    return Err(refused("profil Dziecko jest wyłączony"));
                }
                if for_ms == Some(0) {
                    return Err(refused("czas otwarcia kłódki musi być dodatni"));
                }
                let until_ms = for_ms.map_or(OPEN_UNTIL_CLOSE, |d| now.saturating_add(d));
                Self::require(store, &pin, now)?;
                self.set_ceiling(Some(ChildCeiling {
                    open_until_ms: Some(until_ms),
                    ..ChildCeiling::locked()
                }));
                Ok(())
            }
        }
    }

    fn require(store: &mut ChildStore, pin: &Pin, now: u64) -> Result<(), BrokerError> {
        match store.check(pin, now).map_err(|e| io(&e))? {
            Check::Ok => Ok(()),
            Check::Wrong => Err(refused("błędny PIN rodzica")),
            Check::Blocked(_) => Err(refused("za dużo błędnych prób — PIN chwilowo zablokowany")),
            Check::Unavailable => Err(refused("PIN rodzica nie jest ustawiony")),
        }
    }

    fn set_ceiling(&self, ceiling: Option<ChildCeiling>) {
        self.lock().autonomy.set_child(ceiling);
    }
}
