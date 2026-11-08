//! Atrapa modułu `sessions` (docs/modules/sessions/SPEC.md, „Fake”).
//!
//! - [`FakeSessions`] — sesje w pamięci, deterministyczne identyfikatory (`sess-0001`…) i wirtualny
//!   zegar (+1 s na operację); te same reguły co `sessions-impl` (testy kontraktowe).
//! - [`MemoryKeyVault`] — sejf kluczy w pamięci.
//! - [`TempDbProvider`] — prawdziwe szyfrowane bazy w katalogu tymczasowym (dla testów modułów
//!   `search`/`memory`/`artifacts`, które potrzebują bazy sesji bez `sessions-impl`).
//! - [`fixtures`] — rozmowy z gałęziami i prefiksem barge-in.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod catalog;
pub mod fixtures;
mod history;
mod provider;
mod vault;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, TimeZone, Utc};
use sessions_contract::{KeyVault, SessionError, SessionId, SessionMeta, Turn, TurnId};

pub use provider::TempDbProvider;
pub use vault::MemoryKeyVault;

/// Stan jednej sesji w pamięci.
#[derive(Debug, Clone)]
struct FakeSession {
    meta: SessionMeta,
    turns: BTreeMap<TurnId, Turn>,
    next_branch: u64,
    active_leaf: Option<TurnId>,
    draft: Option<String>,
    unread: u64,
    last_turn_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Default)]
struct State {
    sessions: BTreeMap<SessionId, FakeSession>,
    active: BTreeSet<SessionId>,
    next_session: u64,
    ticks: i64,
}

impl State {
    fn now(&mut self) -> DateTime<Utc> {
        self.ticks += 1;
        let base = Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .single()
            .unwrap_or_default();
        base + chrono::Duration::seconds(self.ticks)
    }

    fn get(&self, id: &SessionId) -> Result<&FakeSession, SessionError> {
        self.sessions
            .get(id)
            .ok_or_else(|| SessionError::NotFound { id: id.clone() })
    }

    fn get_mut(&mut self, id: &SessionId) -> Result<&mut FakeSession, SessionError> {
        self.sessions
            .get_mut(id)
            .ok_or_else(|| SessionError::NotFound { id: id.clone() })
    }
}

impl FakeSession {
    fn turn(&self, turn: TurnId) -> Result<&Turn, SessionError> {
        self.turns
            .get(&turn)
            .ok_or(SessionError::TurnNotFound { turn })
    }

    fn children(&self, parent: Option<TurnId>) -> Vec<TurnId> {
        self.turns
            .values()
            .filter(|t| t.parent == parent)
            .map(|t| t.id)
            .collect()
    }
}

/// Sesje w pamięci (deterministyczne).
pub struct FakeSessions {
    state: Mutex<State>,
    workdir_root: PathBuf,
    vault: Arc<dyn KeyVault>,
}

impl Default for FakeSessions {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeSessions {
    /// Atrapa z katalogiem roboczym `C:\Alfa\Sesje` (tylko ścieżki; nic nie powstaje na dysku)
    /// i sejfem w pamięci.
    pub fn new() -> Self {
        Self::with_vault(Arc::new(MemoryKeyVault::new()))
    }

    /// Atrapa z podanym sejfem (np. do sprawdzenia usuwania kluczy).
    pub fn with_vault(vault: Arc<dyn KeyVault>) -> Self {
        Self {
            state: Mutex::new(State::default()),
            workdir_root: PathBuf::from("C:\\Alfa\\Sesje"),
            vault,
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
