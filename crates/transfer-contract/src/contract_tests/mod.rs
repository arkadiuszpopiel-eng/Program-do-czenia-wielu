//! Współdzielone testy kontraktowe (feature `contract-tests`). Ten sam zestaw uruchamiają
//! `transfer-impl` (ZIP + szyfrowanie, katalogi tymczasowe) i `transfer-fake` (paczki w pamięci).
//! Porty (sesje, dokumenty, sekrety) dostarcza [`Harness`]; stan „świata” porównuje [`world`].

mod cases;
mod fixtures;
mod more;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use accounts_hub_contract::SecretStore;
use sessions_contract::{PortableSession, SessionId, SessionQuery, Sessions};

use crate::Transfer;
use crate::ports::DocumentStore;
use crate::scope::Category;

pub use cases::{
    dry_run_writes_nothing, modes_and_collisions, round_trip_restores_identical_state,
    snapshot_and_rollback,
};
pub use fixtures::{SECRET_PATTERN, SECRET_PLAIN, seed};
pub use more::{
    backup_rotation, encrypted_package, kernel_keys_and_machine_overlay,
    no_secrets_in_plain_export, private_sessions, secrets_never_leave_the_store,
};

/// Środowisko testu: moduł + porty, na których działa.
pub trait Harness {
    /// Moduł.
    fn transfer(&self) -> &dyn Transfer;
    /// Sesje.
    fn sessions(&self) -> &dyn Sessions;
    /// Magazyn kategorii (wszystkie z [`Category::DOCUMENTS`]).
    fn store(&self, category: Category) -> &dyn DocumentStore;
    /// Magazyn sekretów.
    fn secrets(&self) -> &dyn SecretStore;
    /// Ścieżka pliku paczki o nazwie `name` w katalogu roboczym testu.
    fn path(&self, name: &str) -> PathBuf;
    /// Wszystkie wpisy paczki **jawnej** (ścieżka → bajty), łącznie z manifestem.
    fn entries(&self, package: &Path) -> Vec<(String, Vec<u8>)>;
    /// Paczki w katalogu (kopie zapasowe), posortowane.
    fn packages_in(&self, dir: &Path) -> Vec<PathBuf>;
    /// Identyfikator maszyny w portach (`MachineInfo::id`).
    fn machine_id(&self) -> String;
}

/// Stan „świata”: sesje (przenośnie) i dokumenty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    /// Sesje.
    pub sessions: BTreeMap<SessionId, PortableSession>,
    /// Dokumenty.
    pub documents: BTreeMap<(Category, String), Vec<u8>>,
}

/// Wszystkie sesje (także w archiwum i koszu).
pub fn all_session_ids(s: &dyn Sessions) -> Vec<SessionId> {
    let q = SessionQuery {
        include_archived: true,
        ..SessionQuery::default()
    };
    let mut ids: Vec<SessionId> = ok(s.list_sessions(&q))
        .into_iter()
        .map(|x| x.meta.id)
        .collect();
    let trashed = SessionQuery {
        trashed: true,
        ..SessionQuery::default()
    };
    ids.extend(ok(s.list_sessions(&trashed)).into_iter().map(|x| x.meta.id));
    ids.sort();
    ids
}

/// Migawka świata.
pub fn world(h: &dyn Harness) -> World {
    let s = h.sessions();
    let sessions = all_session_ids(s)
        .into_iter()
        .map(|id| {
            let session = PortableSession {
                meta: ok(s.session(&id)),
                turns: ok(s.all_turns(&id)),
                active_leaf: ok(s.active_leaf(&id)),
                draft: ok(s.draft(&id)),
            };
            (id, session)
        })
        .collect();
    let mut documents = BTreeMap::new();
    for category in Category::DOCUMENTS {
        let store = h.store(category);
        for name in ok(store.list()) {
            if let Some(bytes) = ok(store.read(&name)) {
                documents.insert((category, name), bytes);
            }
        }
    }
    World {
        sessions,
        documents,
    }
}

/// Czyści sesje i dokumenty (sekrety zostają — są per maszyna).
pub fn wipe(h: &dyn Harness) {
    for id in all_session_ids(h.sessions()) {
        ok(h.sessions().delete_session(&id));
    }
    for category in Category::DOCUMENTS {
        let store = h.store(category);
        for name in ok(store.list()) {
            ok(store.remove(&name));
        }
    }
}

/// Uruchamia cały zestaw; `factory` daje świeże, puste środowisko.
pub fn run_all<H: Harness>(factory: impl Fn() -> H) {
    let cases: [fn(&dyn Harness); 12] = [
        round_trip_restores_identical_state,
        dry_run_writes_nothing,
        modes_and_collisions,
        snapshot_and_rollback,
        no_secrets_in_plain_export,
        encrypted_package,
        secrets_never_leave_the_store,
        private_sessions,
        kernel_keys_and_machine_overlay,
        backup_rotation,
        more::unknown_session_is_error,
        more::cancel_stops_export,
    ];
    for case in cases {
        let harness = factory();
        case(&harness);
    }
}

/// Wynik albo panika z opisem.
pub(crate) fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("nieoczekiwany błąd: {e}"))
}
