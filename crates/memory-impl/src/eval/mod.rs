//! Zestaw recall@k pamięci (ACCEPTANCE F7-02, `evals/F7/recall/`).
//!
//! - [`format`]: korpus i zapytania jako NDJSON (ten sam format dla zestawu syntetycznego i korpusu
//!   użytkownika), walidacja, JSON Schema;
//! - [`synthetic`]: deterministyczny zestaw PL (≥ 200 zapytań) z danych [`data`];
//! - [`runner`]: ładowanie korpusu i recall@k przez [`memory_contract::MemoryService`].
//!
//! Próg ≥ 0,85 obowiązuje na prawdziwym embedderze (kompozycja `app-*` ze `search_impl` i modelem
//! lokalnym); na CI wynik na `HashEmbedder` jest raportowany, nieblokujący.

pub mod data;
pub mod format;
pub mod runner;
pub mod synthetic;

pub use format::{
    CorpusItem, FormatError, K, MIN_QUERIES, RECALL_THRESHOLD, RecallQuery, RecallSet,
    corpus_schema, queries_schema,
};
pub use runner::{QueryResult, RecallReport, load, run};
pub use synthetic::synthetic_set;
