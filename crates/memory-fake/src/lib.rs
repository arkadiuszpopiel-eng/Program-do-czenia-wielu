//! Atrapa modułu `memory` v0 (docs/modules/memory/SPEC.md, „Fake”).
//!
//! Wpisy w pamięci, `recall` po prostym dopasowaniu słów (bez diakrytyków, prefiksy), wirtualny
//! zegar (+1 s na operację), deterministyczne identyfikatory (`mem-0001`…), licznik kaskady
//! `forget`. Reguły (zakresy, proweniencja, TTL) — wspólne z `memory-impl` (`memory-contract`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, TimeZone, Utc};
use lib_sqlstore::search_tokens;
use memory_contract::{
    ForgetReport, Memory, MemoryEntry, MemoryError, MemoryId, MemoryScope, NewMemory, Recalled,
    RememberMode, SessionId, check_promotion, is_expired, recall_sessions, validate_new,
};

#[derive(Debug, Default)]
struct State {
    entries: BTreeMap<SessionId, BTreeMap<MemoryId, MemoryEntry>>,
    next_id: u64,
    ticks: i64,
    forgotten: u64,
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

    fn entry_mut(
        &mut self,
        scope: &MemoryScope,
        id: &MemoryId,
    ) -> Result<&mut MemoryEntry, MemoryError> {
        let not_found = || MemoryError::NotFound { id: id.clone() };
        let MemoryScope::Session(session) = scope else {
            return Err(not_found());
        };
        self.entries
            .get_mut(session)
            .and_then(|m| m.get_mut(id))
            .ok_or_else(not_found)
    }
}

/// Pamięć w pamięci operacyjnej (deterministyczna).
#[derive(Debug, Default)]
pub struct FakeMemory {
    state: Mutex<State>,
}

impl FakeMemory {
    /// Pusta pamięć.
    pub fn new() -> Self {
        Self::default()
    }

    /// Liczba wykonanych kaskad `forget`.
    pub fn forget_count(&self) -> u64 {
        self.lock().forgotten
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn score(query: &[String], text: &str) -> f32 {
    let terms = search_tokens(text);
    let hits = query
        .iter()
        .filter(|q| terms.iter().any(|t| t.starts_with(q.as_str())))
        .count();
    f32::from(u16::try_from(hits).unwrap_or(u16::MAX))
}

impl Memory for FakeMemory {
    fn remember(&self, new: NewMemory, mode: RememberMode) -> Result<MemoryEntry, MemoryError> {
        let session = validate_new(&new, mode)?;
        let mut st = self.lock();
        st.next_id += 1;
        let entry = MemoryEntry {
            id: MemoryId(format!("mem-{:04}", st.next_id)),
            trusted: new.provenance.is_trusted(),
            scope: new.scope,
            layer: new.layer,
            text: new.text,
            entities: new.entities,
            provenance: new.provenance,
            confidence: new.confidence,
            ttl_secs: new.ttl_secs,
            created_at: st.now(),
            approved: mode == RememberMode::Explicit,
        };
        st.entries
            .entry(session)
            .or_default()
            .insert(entry.id.clone(), entry.clone());
        Ok(entry)
    }

    fn recall(
        &self,
        scopes: &[MemoryScope],
        query: &str,
        k: usize,
    ) -> Result<Vec<Recalled>, MemoryError> {
        let sessions = recall_sessions(scopes)?;
        let terms = search_tokens(query);
        if k == 0 || terms.is_empty() {
            return Ok(Vec::new());
        }
        let mut st = self.lock();
        let now = st.now();
        let mut out: Vec<Recalled> = sessions
            .iter()
            .filter_map(|s| st.entries.get(s))
            .flat_map(BTreeMap::values)
            .filter(|e| e.approved && !is_expired(e, now))
            .map(|e| Recalled {
                score: score(&terms, &e.text),
                entry: e.clone(),
            })
            .collect();
        out.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.entry.id.cmp(&b.entry.id))
        });
        out.truncate(k);
        Ok(out)
    }

    fn get(&self, scope: &MemoryScope, id: &MemoryId) -> Result<MemoryEntry, MemoryError> {
        self.lock().entry_mut(scope, id).map(|e| e.clone())
    }

    fn list(&self, scope: &MemoryScope) -> Result<Vec<MemoryEntry>, MemoryError> {
        let sessions = recall_sessions(std::slice::from_ref(scope))?;
        let st = self.lock();
        Ok(sessions
            .iter()
            .filter_map(|s| st.entries.get(s))
            .flat_map(|m| m.values().cloned())
            .collect())
    }

    fn approve(&self, scope: &MemoryScope, id: &MemoryId) -> Result<MemoryEntry, MemoryError> {
        let mut st = self.lock();
        let entry = st.entry_mut(scope, id)?;
        entry.approved = true;
        Ok(entry.clone())
    }

    fn forget(&self, scope: &MemoryScope, id: &MemoryId) -> Result<ForgetReport, MemoryError> {
        let mut st = self.lock();
        st.entry_mut(scope, id)?;
        if let MemoryScope::Session(s) = scope {
            st.entries.get_mut(s).map(|m| m.remove(id));
        }
        st.forgotten += 1;
        Ok(ForgetReport {
            entry: true,
            fts_rows: 1,
            vectors: 1,
            derived: 0,
        })
    }

    fn promote(
        &self,
        scope: &MemoryScope,
        id: &MemoryId,
        to: MemoryScope,
    ) -> Result<MemoryEntry, MemoryError> {
        let entry = self.get(scope, id)?;
        check_promotion(&entry, &to)?;
        Ok(entry)
    }
}
