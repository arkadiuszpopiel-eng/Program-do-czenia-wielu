//! Magazyn F7 w pamięci (`MemoryBackend`): mapy wpisów, dziennika i eksportów per zakres,
//! wyszukiwanie leksykalne po rdzeniach (bez wektorów), licznik zatarć indeksu.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use memory_contract::rerank::{content_stems, stems_match};
use memory_contract::{
    Candidate, CandidateQuery, ChangeId, CommitReport, DropReport, ExportNote, JournalRecord,
    MemoryBackend, MemoryEntry, MemoryError, MemoryId, MemoryScope, StoreOp,
};

#[derive(Debug, Default, Clone)]
struct ScopeData {
    entries: BTreeMap<MemoryId, MemoryEntry>,
    journal: BTreeMap<ChangeId, JournalRecord>,
    exports: BTreeMap<String, ExportNote>,
}

impl ScopeData {
    fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.journal.is_empty() && self.exports.is_empty()
    }
}

/// Magazyn w pamięci. Zakresy projektu/agentki/globalny „mają własną bazę”: `drop_scope` zgłasza
/// crypto-shredding. Usunięcie wpisu liczy 1 wiersz FTS i 1 wektor (jak indeks `search`).
#[derive(Debug, Default)]
pub struct FakeBackend {
    scopes: Mutex<BTreeMap<MemoryScope, ScopeData>>,
    commits: Mutex<u64>,
    fail_next_commit: Mutex<bool>,
}

impl FakeBackend {
    /// Pusty magazyn.
    pub fn new() -> Self {
        Self::default()
    }

    /// Liczba wykonanych transakcji.
    pub fn commit_count(&self) -> u64 {
        *self.commits.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Następna transakcja zakończy się błędem magazynu (test atomowości).
    pub fn fail_next_commit(&self) {
        *self
            .fail_next_commit
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
    }

    /// Wszystkie teksty w magazynie (wpisy i migawki dziennika) — testy kaskady „nic nie zostało”.
    pub fn all_texts(&self) -> Vec<String> {
        let st = self.lock();
        let mut out = Vec::new();
        for data in st.values() {
            out.extend(data.entries.values().map(|e| e.text.clone()));
            for rec in data.journal.values() {
                out.extend(rec.before.iter().chain(&rec.after).map(|e| e.text.clone()));
            }
        }
        out
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<MemoryScope, ScopeData>> {
        self.scopes.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl MemoryBackend for FakeBackend {
    fn entries(&self, scope: &MemoryScope) -> Result<Vec<MemoryEntry>, MemoryError> {
        let mut out: Vec<MemoryEntry> = self
            .lock()
            .get(scope)
            .map(|d| d.entries.values().cloned().collect())
            .unwrap_or_default();
        out.sort_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
        Ok(out)
    }

    fn entry(
        &self,
        scope: &MemoryScope,
        id: &MemoryId,
    ) -> Result<Option<MemoryEntry>, MemoryError> {
        Ok(self
            .lock()
            .get(scope)
            .and_then(|d| d.entries.get(id))
            .cloned())
    }

    fn search(
        &self,
        scope: &MemoryScope,
        query: &CandidateQuery,
    ) -> Result<Vec<Candidate>, MemoryError> {
        let st = self.lock();
        let Some(data) = st.get(scope) else {
            return Ok(Vec::new());
        };
        let mut out: Vec<Candidate> = data
            .entries
            .values()
            .filter_map(|e| {
                let doc = content_stems(&e.text);
                let hits = query
                    .stems
                    .iter()
                    .filter(|q| doc.iter().any(|d| stems_match(q, d)))
                    .count();
                (hits > 0).then(|| Candidate {
                    id: e.id.clone(),
                    score: f32::from(u16::try_from(hits).unwrap_or(u16::MAX)),
                })
            })
            .collect();
        out.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
        out.truncate(query.limit);
        Ok(out)
    }

    fn commit(&self, scope: &MemoryScope, ops: Vec<StoreOp>) -> Result<CommitReport, MemoryError> {
        let mut fail = self
            .fail_next_commit
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if std::mem::take(&mut *fail) {
            return Err(MemoryError::storage("atrapa: wymuszony błąd transakcji"));
        }
        let mut st = self.lock();
        let mut data = st.get(scope).cloned().unwrap_or_default();
        let mut report = CommitReport::default();
        for op in ops {
            match op {
                StoreOp::Put(e) => {
                    if &e.scope != scope {
                        return Err(MemoryError::invalid("wpis spoza zakresu transakcji"));
                    }
                    data.entries.insert(e.id.clone(), *e);
                }
                StoreOp::Delete(id) => {
                    if data.entries.remove(&id).is_some() {
                        report.entries_deleted += 1;
                        report.fts_rows += 1;
                        report.vectors += 1;
                    }
                }
                StoreOp::PutJournal(r) => {
                    data.journal.insert(r.id.clone(), *r);
                }
                StoreOp::DeleteJournal(id) => {
                    if data.journal.remove(&id).is_some() {
                        report.journal_deleted += 1;
                    }
                }
                StoreOp::NoteExport { name, at } => {
                    data.exports.insert(name.clone(), ExportNote { name, at });
                }
            }
        }
        if data.is_empty() {
            st.remove(scope);
        } else {
            st.insert(scope.clone(), data);
        }
        *self.commits.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        Ok(report)
    }

    fn journal(&self, scope: &MemoryScope) -> Result<Vec<JournalRecord>, MemoryError> {
        let mut out: Vec<JournalRecord> = self
            .lock()
            .get(scope)
            .map(|d| d.journal.values().cloned().collect())
            .unwrap_or_default();
        out.sort_by(|a, b| (b.at, &b.id).cmp(&(a.at, &a.id)));
        Ok(out)
    }

    fn exports(&self, scope: &MemoryScope) -> Result<Vec<ExportNote>, MemoryError> {
        Ok(self
            .lock()
            .get(scope)
            .map(|d| d.exports.values().cloned().collect())
            .unwrap_or_default())
    }

    fn drop_scope(&self, scope: &MemoryScope) -> Result<DropReport, MemoryError> {
        let removed = self.lock().remove(scope).unwrap_or_default();
        let n = removed.entries.len();
        Ok(DropReport {
            commit: CommitReport {
                entries_deleted: n,
                fts_rows: n,
                vectors: n,
                journal_deleted: removed.journal.len(),
            },
            shredded: !matches!(scope, MemoryScope::Session(_)),
        })
    }

    fn scopes(&self) -> Result<Vec<MemoryScope>, MemoryError> {
        Ok(self.lock().keys().cloned().collect())
    }
}
