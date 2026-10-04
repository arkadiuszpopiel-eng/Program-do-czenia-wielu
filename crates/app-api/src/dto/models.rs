//! Menedżer modeli i silników (`models_*`, `embed_model_activate`, `search_reindex_*`, zdarzenia
//! `ModelProgress`, `ModelChanged`, `ReindexStatus`) — odpowiednik `types-models.ts`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::LocalizedText;

/// Rodzaj pozycji katalogu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelItemKind {
    Llm,
    Stt,
    Tts,
    Vad,
    Wake,
    Speaker,
    Embed,
    Sidecar,
}

/// Stan pozycji: brak → (kolejka) → pobieranie → [zgoda TOFU] → instalacja → zainstalowana.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelItemState {
    Missing,
    Queued,
    Downloading,
    /// Przerwane (anulowanie, restart) — plik częściowy czeka na wznowienie.
    Paused,
    /// Pobrane bez przypiętego SHA-256 — czeka na jawną zgodę (karta z policzonym hashem).
    NeedsTrust,
    Installing,
    /// Zainstalowana i zweryfikowana (hash przypięty albo zaakceptowany przy pierwszym użyciu).
    Installed,
    /// Pliki obecne, ale nie z menedżera (skopiowane ręcznie) — bez rekordu weryfikacji.
    External,
    /// Weryfikacja wykryła niezgodny albo brakujący plik.
    Corrupt,
    Failed,
}

/// Plik pozycji (pobierany albo wynikowy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFileView {
    pub name: String,
    pub url: String,
    pub size_bytes: u64,
    /// SHA-256 przypięty w katalogu (`null` — do przypięcia przez człowieka).
    pub pinned_sha256: Option<String>,
    /// SHA-256 policzony przy pobraniu (karta TOFU) albo zapisany przy instalacji.
    pub sha256: Option<String>,
}

/// Postęp pobierania pliku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProgressView {
    pub file: String,
    pub done: u64,
    pub total: Option<u64>,
}

/// Pozycja katalogu z jej stanem na tej maszynie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelItem {
    pub id: String,
    pub kind: ModelItemKind,
    pub name: String,
    /// Licencja (SPDX albo opis warunków).
    pub license: String,
    /// Strona źródła (repozytorium, karta modelu).
    pub source: String,
    pub size_bytes: u64,
    /// Katalog docelowy na tej maszynie.
    pub target: String,
    pub files: Vec<ModelFileView>,
    pub state: ModelItemState,
    /// Wszystkie pliki mają przypięty SHA-256 (bez karty TOFU).
    pub pinned: bool,
    /// Adresy, rozmiary i licencja potwierdzone przez człowieka (inaczej „do potwierdzenia”).
    pub confirmed: bool,
    /// Pozycja do pobrania w aplikacji (inaczej — instalacja ręczna wg opisu).
    pub downloadable: bool,
    pub note: LocalizedText,
    pub progress: Option<ModelProgressView>,
    pub error: Option<String>,
    /// Embedder aktywny w wyszukiwaniu (tylko `embed`).
    pub active: bool,
}

/// Embedder wyszukiwania (`[search.embedder] model`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedderView {
    /// Wybór w ustawieniach (`lexical` albo identyfikator modelu).
    pub configured: String,
    /// Model w użyciu (`lexical`, gdy wybrany model nie jest zainstalowany albo się nie załadował).
    pub active: String,
    /// Identyfikator w indeksie (`model_id/wymiar` — zmiana = przebudowa wektorów).
    pub index_id: String,
    pub dims: u32,
    pub error: Option<String>,
}

/// Przebudowa wektorów w tle (bez treści: tylko liczniki).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReindexView {
    pub running: bool,
    pub embedder: String,
    pub databases: u64,
    pub rebuilt: u64,
    pub embedded: u64,
    /// Postęp bieżącej bazy.
    pub done: u64,
    pub total: u64,
    pub failed: u64,
    pub cancelled: bool,
    pub finished: bool,
}

/// Lista pozycji, embedder i przebudowa (`models_list`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelsView {
    pub items: Vec<ModelItem>,
    pub embedder: EmbedderView,
    pub reindex: ReindexView,
    /// Limit równoległych pobrań.
    pub parallel: u32,
}

/// Zgoda TOFU: SHA-256 plików pokazane na karcie (nazwa pliku → hash).
pub type TrustedHashes = BTreeMap<String, String>;
