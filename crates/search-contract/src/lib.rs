//! Kontrakt modułu `search` (docs/modules/search/SPEC.md, ADR 0008).
//!
//! Indeks żyje **w szyfrowanej bazie sesji** (FTS5 z kolumną złożoną `fold_pl` + `vec0`), więc
//! usunięcie sesji (pliku i klucza) unieważnia także indeks. Zapytania: pełnotekstowe z
//! podświetleniem fragmentów, kNN po embeddingu ([`Embedder`]) i hybryda (RRF).
//!
//! **Wyszukiwanie między sesjami** ([`SessionSet::Many`]/[`SessionSet::All`]) jest funkcją UI
//! wykonywaną jako właściciel ([`Caller::Owner`], `Ctrl+Shift+F`) i otwiera wiele baz.
//! **Nigdy nie jest narzędziem agentki** — [`Caller::Agent`] widzi wyłącznie własną sesję
//! (PLAN §10, [`authorize`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod api;
mod error;
mod rules;
mod snippet;
mod types;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use api::{Embedder, Search, TxIndexer, TxSearcher};
pub use error::SearchError;
pub use rules::{MAX_LIMIT, RRF_K, authorize, fuse_rrf, sort_hits};
pub use snippet::{DEFAULT_SNIPPET_CHARS, make_snippet};
pub use types::{
    Caller, ConnQuery, Doc, DocId, DocKind, Highlight, Hit, Mode, Query, RemoveReport, SessionSet,
    Snippet,
};

pub use core_bus_contract::SessionId;

/// Nazwy zdarzeń modułu. Ładunki bez treści zapytań i dokumentów (tylko liczniki).
pub mod events {
    /// Zapytanie wykonane (poziom Diagnostics: tryb, liczba sesji, liczba trafień, czas).
    pub const QUERY: &str = "search.query";
    /// Dokument usunięty z indeksu (kaskada `forget`/usunięcia).
    pub const REMOVED: &str = "search.removed";
    /// Embedder załadowany.
    pub const EMBEDDER_LOADED: &str = "search.embedder.loaded";
}
