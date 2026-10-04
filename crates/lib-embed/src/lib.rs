//! Lokalny embedder tekstu ONNX dla modułu `search` (PLAN §10, ADR 0004, ACCEPTANCE F7-02).
//!
//! Wspólna biblioteka (`lib-*`, bez logiki modułu): zależy tylko od `search-contract`
//! (trait [`Embedder`]) i `model-residency-contract` (dzierżawa RAM). Kompozycja `app-*` podaje
//! [`OnnxEmbedder`] do `search_impl::SqliteSearch` zamiast osadzacza leksykalnego.
//!
//! - [`EmbedManifest`] (`alfa-embed-v1`): pliki ONNX i `tokenizer.json` z SHA-256 (inny plik nie jest
//!   ładowany), wymiar zgodny z `vec0`, limit tokenów, pooling (średni/CLS/gotowy), prefiksy E5;
//! - [`TextTokenizer`]: własny tokenizer `tokenizer.json` (SentencePiece Unigram: XLM-R,
//!   `multilingual-e5-*`), zgodny z HF `tokenizers` na wektorach referencyjnych;
//! - model przez `tract-onnx` (czysty Rust, wymiary symboliczne), pooling + normalizacja L2;
//! - [`OnnxEmbedder`]: wątek tła (leniwe ładowanie, zwalnianie po bezczynności, dzierżawa
//!   `model-residency`), wsady z limitem, prefiksy `query:`/`passage:`;
//! - [`catalog`] + [`install`]: znane modele (URL, rozmiar, licencja, SHA-256) i instalator
//!   z wznawianiem i weryfikacją przez port pobierania (adapter HTTP w `app-*`);
//! - `testkit` (feature): zabawkowy model ONNX + tokenizer do testów bez pobierania.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod catalog;
mod embedder;
mod engine;
mod error;
pub mod install;
pub mod manifest;
mod model;
mod pool;
mod residency;
#[cfg(feature = "testkit")]
pub mod testkit;
pub mod tokenizer;
mod worker;

pub use catalog::{CATALOG, CatalogEntry, DEFAULT_MODEL};
pub use embedder::{MAX_TEXTS_PER_JOB, OnnxEmbedder, OnnxEmbedderBuilder};
pub use engine::Engine;
pub use error::EmbedError;
pub use install::{Fetched, Fetcher, HashPolicy, InstallProgress, Installed, install, installed};
pub use manifest::{EMBED_MANIFEST_FORMAT, EmbedManifest, FileRef, MANIFEST_FILE};
pub use model::{OnnxModel, Output};
pub use pool::{Pooling, l2_normalize, mean_pool};
pub use residency::{RESIDENCY_OWNER, lease_request};
pub use search_contract::Embedder;
pub use tokenizer::TextTokenizer;
pub use worker::StatsSnapshot;
