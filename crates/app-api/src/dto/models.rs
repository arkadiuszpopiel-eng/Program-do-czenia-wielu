//! Menedżer modeli i silników (`models_*`, pakiety `models_bundle*`, `embed_model_activate`,
//! `search_reindex_*`, zdarzenia `ModelProgress`, `ModelChanged`, `ReindexStatus`) — odpowiednik
//! `types-models.ts`.

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

/// Stan pakietu na tej maszynie (wyliczany ze stanów jego pozycji).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleState {
    /// Żadna pozycja nie jest zainstalowana.
    NotInstalled,
    /// Część pozycji zainstalowana (albo pobieranie wstrzymane).
    Partial,
    /// Wszystkie pozycje zainstalowane (także skopiowane ręcznie).
    Installed,
    /// Pozycja uszkodzona (weryfikacja) albo z błędem pobierania lub instalacji — do naprawy.
    Corrupt,
    /// Pozycja w kolejce, pobierana albo instalowana.
    Downloading,
    /// Pobrane pozycje bez przypiętego SHA-256 czekają na zgodę (karta TOFU).
    NeedsTrust,
}

/// Dopasowanie pakietu do sprzętu tej maszyny.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleFitKind {
    Fits,
    /// Zadziała z kompromisem (np. część modelu na procesorze — wolniej).
    Tight,
    TooWeak,
}

/// Dopasowanie z uzasadnieniem (`reason` — dla `tight` i `too_weak`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleFit {
    pub kind: BundleFitKind,
    pub reason: Option<LocalizedText>,
}

/// Wymagania pakietu. Progi w MB z tolerancją raportowania systemu (np. 16 GB RAM ≈ ≥ 15 000 MB,
/// karta 8 GB ≈ ≥ 7 500 MB); `text` — opis do pokazania z wartościami nominalnymi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleRequirements {
    pub min_ram_mb: u64,
    /// Pamięć karty graficznej (CUDA albo Vulkan); `null` — karta niepotrzebna.
    pub min_vram_mb: Option<u64>,
    /// Rdzenie fizyczne procesora; `null` — bez wymagania.
    pub min_cpu_cores: Option<u32>,
    /// Karta konieczna; inaczej `min_vram_mb` i `min_cpu_cores` to alternatywy („karta albo procesor”).
    pub gpu_required: bool,
    pub text: LocalizedText,
}

/// Pozycja pakietu (wariant silnika dobrany dla tej maszyny) z jej stanem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleItemView {
    pub id: String,
    pub name: String,
    pub kind: ModelItemKind,
    pub state: ModelItemState,
    pub size_bytes: u64,
    /// Do pobrania w aplikacji (inaczej — instalacja ręczna; pakiet jej nie pobiera).
    pub downloadable: bool,
    /// Silnik zapasowy (CPU) — używany, gdy wersja na kartę graficzną nie wystartuje.
    pub fallback: bool,
}

/// Uwaga o jakości: zalecenie albo metoda pomiaru według normy (bez deklaracji certyfikacji).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityNote {
    pub aspect: LocalizedText,
    /// Norma albo metoda (np. `ITU-T P.800 / P.808`).
    pub standard: String,
    pub text: LocalizedText,
}

/// Pakiet „Modele i silniki” w skali ocen 1–6 (6 — wzorcowy, 1 — minimalny) dla tej maszyny.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelBundle {
    pub id: String,
    /// Ocena 1–6.
    pub rating: u8,
    pub name: LocalizedText,
    pub summary: LocalizedText,
    pub requirements: BundleRequirements,
    pub items: Vec<BundleItemView>,
    /// Łączny rozmiar pobrań pozycji.
    pub size_bytes: u64,
    /// Co najwyżej tyle zostało do pobrania (pozycje brakujące, wstrzymane, uszkodzone).
    pub missing_bytes: u64,
    pub installed: u32,
    pub total: u32,
    pub state: BundleState,
    pub fit: BundleFit,
    /// Najwyższy pakiet, który pasuje do tej maszyny bez kompromisów.
    pub recommended: bool,
    pub quality: Vec<QualityNote>,
}
