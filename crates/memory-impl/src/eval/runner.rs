//! Runner recall@k: ładuje korpus do pamięci (jako właściciel), wykonuje zapytania przez
//! [`MemoryService::recall_as`] (hybryda + reranking) i liczy recall@k, trafienie@1 i MRR.

use std::collections::BTreeMap;

use memory_contract::{
    Accessor, MemoryError, MemoryService, NewMemory, Provenance, RecallRequest, RememberMode,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::format::{RecallSet, scope_of};

/// Wynik jednego zapytania (linia `results.ndjson`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QueryResult {
    /// Zapytanie.
    pub id: String,
    /// Rodzaj.
    pub kind: String,
    /// Odsetek oczekiwanych wpisów w top-k (0–1).
    pub recall: f64,
    /// Pozycja pierwszego oczekiwanego wpisu (od 1), jeśli w top-k.
    pub rank: Option<usize>,
    /// Identyfikatory korpusu w top-k.
    pub top: Vec<String>,
}

/// Raport zestawu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecallReport {
    /// `k`.
    pub k: usize,
    /// Liczba zapytań.
    pub queries: usize,
    /// Średni recall@k.
    pub recall_at_k: f64,
    /// Odsetek zapytań z oczekiwanym wpisem na pozycji 1.
    pub hit_at_1: f64,
    /// Mean Reciprocal Rank (w top-k).
    pub mrr: f64,
    /// recall@k per rodzaj zapytania.
    pub by_kind: BTreeMap<String, f64>,
    /// Zapytania bez żadnego oczekiwanego wpisu w top-k.
    pub misses: Vec<String>,
}

impl RecallReport {
    /// Czy spełnia próg (domyślnie [`super::RECALL_THRESHOLD`]).
    pub fn passes(&self, threshold: f64) -> bool {
        self.recall_at_k >= threshold
    }
}

fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 {
        return 0.0;
    }
    f64::from(u32::try_from(a).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(b).unwrap_or(u32::MAX))
}

/// Ładuje korpus (właściciel, `Explicit`); zwraca mapę `id korpusu → id wpisu`.
pub fn load(
    memory: &dyn MemoryService,
    set: &RecallSet,
) -> Result<BTreeMap<String, String>, MemoryError> {
    let mut ids = BTreeMap::new();
    for item in &set.corpus {
        let scope = scope_of(&item.scope)
            .ok_or_else(|| MemoryError::invalid(format!("zakres `{}`", item.scope)))?;
        let new = NewMemory {
            subject: item.subject.clone(),
            ..NewMemory::new(scope, item.layer, item.text.clone(), Provenance::User)
        };
        let entry = memory.remember_as(&Accessor::Owner, new, RememberMode::Explicit)?;
        ids.insert(entry.id.0, item.id.clone());
    }
    Ok(ids)
}

/// Uruchamia zestaw na załadowanej pamięci.
pub fn run(
    memory: &dyn MemoryService,
    set: &RecallSet,
    loaded: &BTreeMap<String, String>,
    k: usize,
) -> Result<(RecallReport, Vec<QueryResult>), MemoryError> {
    let mut results = Vec::new();
    for q in &set.queries {
        let scopes = q.scopes.iter().filter_map(|s| scope_of(s)).collect();
        let req = RecallRequest {
            layers: Vec::new(),
            ..RecallRequest::new(scopes, q.query.clone(), k)
        };
        let top: Vec<String> = memory
            .recall_as(&Accessor::Owner, &req)?
            .into_iter()
            .filter_map(|r| loaded.get(&r.entry.id.0).cloned())
            .collect();
        let found = q.expected.iter().filter(|e| top.contains(e)).count();
        let rank = top
            .iter()
            .position(|t| q.expected.contains(t))
            .map(|p| p + 1);
        results.push(QueryResult {
            id: q.id.clone(),
            kind: q.kind.clone(),
            recall: ratio(found, q.expected.len()),
            rank,
            top,
        });
    }
    let n = results.len();
    let sum = |f: &dyn Fn(&QueryResult) -> f64| -> f64 { results.iter().map(f).sum::<f64>() };
    let mean = |total: f64| if n == 0 { 0.0 } else { total / ratio(n, 1) };
    let mut by_kind: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    for r in &results {
        let slot = by_kind.entry(r.kind.clone()).or_default();
        slot.0 += r.recall;
        slot.1 += 1;
    }
    let report = RecallReport {
        k,
        queries: n,
        recall_at_k: mean(sum(&|r| r.recall)),
        hit_at_1: mean(sum(&|r| if r.rank == Some(1) { 1.0 } else { 0.0 })),
        mrr: mean(sum(&|r| r.rank.map_or(0.0, |p| 1.0 / ratio(p, 1)))),
        by_kind: by_kind
            .into_iter()
            .map(|(kind, (total, count))| (kind, total / ratio(count, 1)))
            .collect(),
        misses: results
            .iter()
            .filter(|r| r.rank.is_none())
            .map(|r| r.id.clone())
            .collect(),
    };
    Ok((report, results))
}
