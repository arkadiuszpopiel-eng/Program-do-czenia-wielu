//! Profil Dziecko w Sędzim (plan 50 §4.4, Z.6): kłódka jako sufit w tabeli autonomii — obowiązuje
//! każde działanie, niezależnie od tego, co pokazuje interfejs. PIN rodzica sprawdza tylko Sędzia;
//! w Audycie zostaje rodzaj zmiany i wynik, nigdy PIN. Zamknięcie kłódki unieważnia tokeny, plany
//! i karty wydane bez niej (`State::sync_child`).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, MutexGuard};

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

/// Połączenie aplikacji (rola Jądra) z Sędzią. Gdy rozłączy się ostatnie — Alfa zamknięta albo
/// padła — kłódka otwarta „do zamknięcia Alfy” zamyka się (Sędzia jako usługa działa dalej).
pub struct CoreSession(Arc<BrokerEngine>);

impl CoreSession {
    /// Liczy połączenie aplikacji do chwili zwolnienia.
    pub fn new(engine: Arc<BrokerEngine>) -> Self {
        engine.core_clients.fetch_add(1, Ordering::SeqCst);
        Self(engine)
    }
}

impl Drop for CoreSession {
    fn drop(&mut self) {
        if self.0.core_clients.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.0.close_until_exit();
        }
    }
}

impl BrokerEngine {
    /// Profil Dziecko z pliku `path` (prywatny katalog Sędziego); włączony albo uszkodzony zapis
    /// zamyka kłódkę od startu. `tamper_resistant` — katalog niedostępny dla konta dziecka
    /// (usługa na osobnym koncie); inaczej interfejs mówi, że ochrona jest umowna.
    #[must_use]
    pub fn with_child_lock(self, path: PathBuf, params: PinParams, tamper_resistant: bool) -> Self {
        let mut store = ChildStore::load(Some(path), params);
        store.tamper_resistant = tamper_resistant;
        let locks = store.locks();
        *self.child_store() = store;
        let mut st = self.lock();
        st.autonomy.set_child(locks.then(ChildCeiling::locked));
        st.unrestricted = !locks;
        drop(st);
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
            tamper_resistant: store.tamper_resistant,
            unknown: false,
            transcripts: store.record.transcripts,
        }
    }

    /// `Broker::change_child_lock`. Kolejność blokad: profil, potem stan (jak w `child_view`).
    /// Poszerzenie (otwarcie, wyłączenie, nowy PIN) obowiązuje dopiero po zapisie w Audycie;
    /// zawężenie (zamknięcie, włączenie) — także bez niego.
    pub(crate) fn change_child_sync(
        &self,
        change: ChildLockChange,
    ) -> Result<ChildLockView, BrokerError> {
        let now = self.now();
        let mut store = self.child_store();
        let (op, widening) = match &change {
            ChildLockChange::SetPin { .. } => ("set_pin", true),
            ChildLockChange::Enable { on: true, .. } => ("enable", false),
            ChildLockChange::Enable { on: false, .. } => ("disable", true),
            ChildLockChange::Open { .. } => ("open", true),
            ChildLockChange::Close => ("close", false),
            ChildLockChange::Verify { .. } => ("verify", true),
            ChildLockChange::Transcripts { on: true, .. } => ("transcripts_on", true),
            ChildLockChange::Transcripts { on: false, .. } => ("transcripts_off", true),
        };
        match self.apply_child(&mut store, change, now, op) {
            Ok(()) if !widening => {
                drop(store);
                self.audit(EVENT_CHILD_LOCK, None, json!({ "op": op, "ok": true }))?;
            }
            Ok(()) => {
                drop(store);
                // Zmiana już obowiązuje (zgoda zapisana przed nią) — wynik dopisany, gdy Audyt działa.
                let _ = self.audit(EVENT_CHILD_LOCK, None, json!({ "op": op, "ok": true }));
            }
            Err(e) => {
                drop(store);
                let _ = self.audit(EVENT_CHILD_LOCK, None, json!({ "op": op, "ok": false }));
                return Err(e);
            }
        }
        Ok(self.child_view())
    }

    /// Zgoda na poszerzenie (PIN sprawdzony) w Audycie, zanim zacznie obowiązywać; wynik (`ok`) dopisuje
    /// `change_child_sync` po zmianie — nieudany zapis pliku daje „zgoda”, potem „błąd”, nigdy „ok”.
    fn granted(&self, op: &str) -> Result<(), BrokerError> {
        self.audit(
            EVENT_CHILD_LOCK,
            None,
            json!({ "op": op, "stage": "granted" }),
        )
    }

    fn apply_child(
        &self,
        store: &mut ChildStore,
        change: ChildLockChange,
        now: u64,
        op: &str,
    ) -> Result<(), BrokerError> {
        match change {
            ChildLockChange::Verify { pin } => {
                Self::require(store, &pin, now)?;
                self.granted(op)
            }
            ChildLockChange::Transcripts { pin, on } => {
                Self::require(store, &pin, now)?;
                // W obie strony najpierw Audyt — dziecko i rodzic widzą w dzienniku, kto i kiedy.
                self.granted(op)?;
                let before = store.record.transcripts;
                store.record.transcripts = on;
                store.save().map_err(|e| {
                    store.record.transcripts = before;
                    io(&e)
                })
            }
            ChildLockChange::Close => {
                if store.locks() {
                    self.set_ceiling(Some(ChildCeiling::locked()), now);
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
                self.granted(op)?;
                store.set_pin(&new).map_err(|e| io(&e))
            }
            ChildLockChange::Enable { pin, on } => {
                Self::require(store, &pin, now)?;
                if !on {
                    self.granted(op)?;
                }
                let (before, reads) = (store.record.enabled, store.record.transcripts);
                store.record.enabled = on;
                // Wyłączenie profilu kończy też pełną treść w dzienniku — po ponownym włączeniu
                // rodzic decyduje o niej od nowa (znak dla dziecka zależy od obu).
                store.record.transcripts &= on;
                if let Err(e) = store.save() {
                    // Włączenie obowiązuje i bez zapisu (bezpieczny kierunek); wyłączenie — nie.
                    store.record.enabled = before || on;
                    store.record.transcripts = reads;
                    if store.record.enabled {
                        self.set_ceiling(Some(ChildCeiling::locked()), now);
                    }
                    return Err(io(&e));
                }
                self.set_ceiling(on.then(ChildCeiling::locked), now);
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
                self.granted(op)?;
                self.set_ceiling(
                    Some(ChildCeiling {
                        open_until_ms: Some(until_ms),
                        ..ChildCeiling::locked()
                    }),
                    now,
                );
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

    /// Nowy sufit; przejście w kłódkę zamkniętą od razu unieważnia wydane bez niej tokeny i karty.
    fn set_ceiling(&self, ceiling: Option<ChildCeiling>, now: u64) {
        let mut st = self.lock();
        st.autonomy.set_child(ceiling);
        st.sync_child(now);
    }

    /// Alfa zamknięta: kłódka otwarta „do zamknięcia Alfy” zamyka się.
    fn close_until_exit(&self) {
        let now = self.now();
        let mut st = self.lock();
        let open_until_exit = st
            .autonomy
            .child()
            .is_some_and(|c| c.open_until_ms == Some(OPEN_UNTIL_CLOSE));
        if open_until_exit {
            st.autonomy.set_child(Some(ChildCeiling::locked()));
            st.sync_child(now);
            drop(st);
            let _ = self.audit(
                EVENT_CHILD_LOCK,
                None,
                json!({ "op": "close_on_exit", "ok": true }),
            );
        }
    }
}
