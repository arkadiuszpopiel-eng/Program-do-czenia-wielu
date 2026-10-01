//! Format zestawu recall (`evals/F7/recall/`): korpus i zapytania jako NDJSON, walidacja.

use std::collections::BTreeSet;

use memory_contract::{Layer, MemoryScope, parse_scope_key, validate_scope};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Minimalna liczba zapytań zestawu akceptacyjnego (ACCEPTANCE F7-02).
pub const MIN_QUERIES: usize = 200;
/// Próg recall@5 na prawdziwym embedderze (ACCEPTANCE F7-02).
pub const RECALL_THRESHOLD: f64 = 0.85;
/// `k` w recall@k.
pub const K: usize = 5;

/// Wpis korpusu (linia `corpus.ndjson`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CorpusItem {
    /// Identyfikator (`[A-Za-z0-9_-]`, unikalny).
    pub id: String,
    /// Zakres: `global`, `project:<id>`, `agent:<id>`, `session:<id>`.
    pub scope: String,
    /// Warstwa (`semantic`, `episodic`, `procedural`).
    pub layer: Layer,
    /// Treść.
    pub text: String,
    /// Temat (opcjonalnie).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// Zapytanie (linia `queries.ndjson`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecallQuery {
    /// Identyfikator (unikalny).
    pub id: String,
    /// Tekst zapytania (PL).
    pub query: String,
    /// Oczekiwane wpisy korpusu (co najmniej jeden).
    pub expected: Vec<String>,
    /// Przeszukiwane zakresy (co najmniej jeden).
    pub scopes: Vec<String>,
    /// Rodzaj: `pytanie`, `slowa_kluczowe`, `bez_znakow` albo własny.
    pub kind: String,
}

/// Zestaw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecallSet {
    /// Korpus.
    pub corpus: Vec<CorpusItem>,
    /// Zapytania.
    pub queries: Vec<RecallQuery>,
}

/// Błąd formatu (plik, linia od 1, opis).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{file}:{line}: {reason}")]
pub struct FormatError {
    /// Plik.
    pub file: String,
    /// Linia (0 = cały zestaw).
    pub line: usize,
    /// Opis.
    pub reason: String,
}

fn parse_lines<T: serde::de::DeserializeOwned>(
    file: &str,
    text: &str,
) -> Result<Vec<T>, FormatError> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        out.push(serde_json::from_str(trimmed).map_err(|e| FormatError {
            file: file.into(),
            line: i + 1,
            reason: e.to_string(),
        })?);
    }
    Ok(out)
}

/// Zakres z klucza (z walidacją identyfikatora).
pub fn scope_of(key: &str) -> Option<MemoryScope> {
    parse_scope_key(key).filter(|s| validate_scope(s).is_ok())
}

impl RecallSet {
    /// Czyta zestaw z tekstów NDJSON (puste linie i `#…` pomijane) i waliduje.
    pub fn parse(corpus: &str, queries: &str) -> Result<Self, FormatError> {
        let set = Self {
            corpus: parse_lines("corpus.ndjson", corpus)?,
            queries: parse_lines("queries.ndjson", queries)?,
        };
        set.validate()?;
        Ok(set)
    }

    /// Koduje zestaw jako `(corpus.ndjson, queries.ndjson)`.
    pub fn to_ndjson(&self) -> (String, String) {
        let lines = |items: Vec<String>| items.into_iter().map(|l| l + "\n").collect::<String>();
        let corpus = self
            .corpus
            .iter()
            .filter_map(|c| serde_json::to_string(c).ok())
            .collect();
        let queries = self
            .queries
            .iter()
            .filter_map(|q| serde_json::to_string(q).ok())
            .collect();
        (lines(corpus), lines(queries))
    }

    /// Reguły: identyfikatory unikalne i przenośne, zakresy poprawne, treści niepuste, oczekiwane
    /// wpisy istnieją i leżą w przeszukiwanych zakresach, warstwa robocza poza zestawem.
    pub fn validate(&self) -> Result<(), FormatError> {
        let err = |file: &str, line: usize, reason: String| FormatError {
            file: file.into(),
            line,
            reason,
        };
        let id_ok = |id: &str| {
            !id.is_empty()
                && id.len() <= 64
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        };
        let mut ids = BTreeSet::new();
        for (i, c) in self.corpus.iter().enumerate() {
            let line = i + 1;
            if !id_ok(&c.id) || !ids.insert(c.id.as_str()) {
                return Err(err(
                    "corpus.ndjson",
                    line,
                    format!("identyfikator `{}`", c.id),
                ));
            }
            if scope_of(&c.scope).is_none() {
                return Err(err("corpus.ndjson", line, format!("zakres `{}`", c.scope)));
            }
            if c.text.trim().is_empty() || c.layer == Layer::Working {
                return Err(err("corpus.ndjson", line, "treść lub warstwa".into()));
            }
        }
        let mut qids = BTreeSet::new();
        for (i, q) in self.queries.iter().enumerate() {
            let line = i + 1;
            if !id_ok(&q.id) || !qids.insert(q.id.as_str()) || q.query.trim().is_empty() {
                return Err(err("queries.ndjson", line, format!("zapytanie `{}`", q.id)));
            }
            if q.scopes.is_empty() || q.scopes.iter().any(|s| scope_of(s).is_none()) {
                return Err(err("queries.ndjson", line, "zakresy".into()));
            }
            if q.expected.is_empty() {
                return Err(err(
                    "queries.ndjson",
                    line,
                    "brak oczekiwanych wpisów".into(),
                ));
            }
            for e in &q.expected {
                let Some(item) = self.corpus.iter().find(|c| &c.id == e) else {
                    return Err(err("queries.ndjson", line, format!("nieznany wpis `{e}`")));
                };
                if !q.scopes.contains(&item.scope) {
                    return Err(err(
                        "queries.ndjson",
                        line,
                        format!("wpis `{e}` poza zakresami zapytania"),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Czy zestaw spełnia minimalną liczność zestawu akceptacyjnego.
    pub fn is_acceptance_sized(&self) -> bool {
        self.queries.len() >= MIN_QUERIES
    }
}

/// JSON Schema linii korpusu.
pub fn corpus_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(CorpusItem)).unwrap_or_default()
}

/// JSON Schema linii zapytań.
pub fn queries_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(RecallQuery)).unwrap_or_default()
}
