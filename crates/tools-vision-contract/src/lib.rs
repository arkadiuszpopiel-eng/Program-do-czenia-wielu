//! Kontrakt `tools-vision` (docs/modules/tools-vision/SPEC.md, PLAN §7.1–7.2, §5.4; THREAT_MODEL S26).
//!
//! Trasa „wizja” dla agentek, zbudowana na zrzutach z maskowaniem (`tools-screen`):
//! - `vision_ocr` — tekst ze zrzutu okna/monitora/obszaru albo z pliku obrazu przez [`OcrPort`]
//!   (Windows: `Windows.Media.Ocr` w `platform-windows-ocr-impl`; atrapa deterministyczna),
//!   ze współrzędnymi linii na ekranie (do kliknięcia przez `tools-input`);
//! - `vision_describe` — opis obrazu przez model z wizją ([`DescribePort`]: Router, klasa
//!   „GUI/wizja”); sesja prywatna albo „tylko lokalnie” → wyłącznie model lokalny albo odmowa.
//!
//! Maskowanie (okna Alfy/Brokera, aplikacje z deny-listy zrzutów i dostawców planów, pola haseł)
//! odbywa się w porcie zrzutów **przed** OCR i przed wysłaniem obrazu do modelu; pliki — deny-lista
//! ścieżek przed Brokerem i wymiary z nagłówka przed dekodowaniem (bomba dekompresyjna). Wynik
//! jest treścią niezaufaną (taint `Screen` albo `File`), nigdy w zdarzeniach ani logach.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod args;

pub use args::{
    DescribeArgs, DescribeOutput, ImageSpec, OcrArgs, OcrLineOut, OcrOutput, SourceArg, check_args,
    describe_manifest, manifests, ocr_manifest, sample_args, screen_lines, valid_language,
};

use async_trait::async_trait;
use providers_contract::CancellationToken;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Zdarzenie: OCR wykonany (źródło, wymiary, liczba linii i masek — bez tekstu).
pub const EVENT_OCR: &str = "tool.vision.ocr";
/// Zdarzenie: opis obrazu (źródło, model, lokalny — bez treści).
pub const EVENT_DESCRIBE: &str = "tool.vision.describe";

/// Żądanie OCR: obraz zakodowany (PNG, JPEG, BMP, GIF, WebP — dekoduje port) i język BCP-47
/// (`None` = języki profilu użytkownika).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrRequest {
    /// Bajty obrazu.
    pub image: Vec<u8>,
    /// Język rozpoznawania (np. `pl`, `en-US`).
    pub language: Option<String>,
}

/// Prostokąt w pikselach obrazu wejściowego.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OcrRect {
    /// Lewa krawędź.
    pub x: f32,
    /// Górna krawędź.
    pub y: f32,
    /// Szerokość.
    pub width: f32,
    /// Wysokość.
    pub height: f32,
}

/// Rozpoznane słowo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OcrWord {
    /// Tekst.
    pub text: String,
    /// Położenie (piksele obrazu wejściowego).
    pub rect: OcrRect,
}

/// Rozpoznana linia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OcrLine {
    /// Tekst linii.
    pub text: String,
    /// Słowa linii.
    pub words: Vec<OcrWord>,
}

/// Wynik OCR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OcrText {
    /// Język, którym rozpoznawano.
    pub language: String,
    /// Linie w kolejności czytania.
    pub lines: Vec<OcrLine>,
    /// Kąt pochylenia tekstu (stopnie), jeśli wykryty.
    pub angle: Option<f64>,
}

/// Błąd OCR.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OcrError {
    /// OCR niedostępny na tej platformie.
    #[error("OCR niedostępny: {0}")]
    Unsupported(String),
    /// Brak pakietu językowego.
    #[error("brak pakietu językowego OCR: {0}")]
    Language(String),
    /// Obrazu nie da się odczytać (format, uszkodzenie).
    #[error("nie udało się odczytać obrazu: {0}")]
    Image(String),
    /// Obraz ponad limit portu.
    #[error("obraz za duży do OCR: {width}×{height} px")]
    TooLarge {
        /// Szerokość.
        width: u32,
        /// Wysokość.
        height: u32,
    },
    /// Rozpoznawanie się nie powiodło.
    #[error("rozpoznawanie: {0}")]
    Failed(String),
}

/// Port OCR (Windows: `Windows.Media.Ocr`; atrapa: deterministyczna). Wywołania blokujące —
/// z kodu async przez `spawn_blocking`.
pub trait OcrPort: Send + Sync {
    /// Rozpoznaje tekst. Współrzędne w pikselach obrazu wejściowego (port skaluje do swojego
    /// limitu wymiarów i przelicza współrzędne z powrotem).
    fn recognize(&self, request: &OcrRequest) -> Result<OcrText, OcrError>;
    /// Dostępne języki rozpoznawania (tagi BCP-47).
    fn languages(&self) -> Result<Vec<String>, OcrError>;
}

/// Prywatność opisu obrazu wynikająca z tagu sesji.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisionPrivacy {
    /// Zwykła sesja — Router wybiera trasę dozwoloną dla klasy „GUI/wizja”.
    Normal,
    /// Sesja prywatna albo „tylko lokalnie” (i sesja nieznana — fail-closed): wyłącznie model
    /// lokalny, bez niego — odmowa.
    #[default]
    LocalOnly,
}

/// Tag prywatności sesji (kompozycja aplikacji: magazyn sesji).
pub trait PrivacyLookup: Send + Sync {
    /// Prywatność opisu obrazu dla sesji (nieznana sesja → [`VisionPrivacy::LocalOnly`]).
    fn vision_privacy(&self, session: &str) -> VisionPrivacy;
}

/// Żądanie opisu obrazu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribeRequest {
    /// Bajty obrazu (już zamaskowanego, jeśli to zrzut).
    pub image: Vec<u8>,
    /// Typ MIME (`image/png`, `image/jpeg`, `image/gif`, `image/webp`).
    pub media_type: String,
    /// Pytanie agentki o obraz (zwykły tekst, obcięty).
    pub question: Option<String>,
    /// Prywatność sesji.
    pub privacy: VisionPrivacy,
    /// Sesja (metadane żądania).
    pub session: String,
    /// Limit tokenów odpowiedzi.
    pub max_tokens: u32,
}

/// Opis obrazu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    /// Tekst opisu (treść pochodna obrazu — niezaufana).
    pub text: String,
    /// Model (`dostawca:model` albo `auto`).
    pub model: String,
    /// Czy opis powstał lokalnie (bez ruchu sieciowego).
    pub local: bool,
}

/// Błąd opisu obrazu.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DescribeError {
    /// Sesja prywatna, a nie ma lokalnego modelu z wizją.
    #[error("sesja prywatna — opis obrazu tylko lokalnym modelem z wizją, a takiego nie ma")]
    PrivateNoLocal,
    /// Brak modelu z wizją dla tej sesji.
    #[error("brak modelu z obsługą obrazów: {0}")]
    NoVisionModel(String),
    /// Błąd dostawcy.
    #[error("model: {0}")]
    Provider(String),
    /// Anulowano.
    #[error("anulowano")]
    Cancelled,
}

/// Port opisu obrazu (aplikacja: Router dla klasy „GUI/wizja”, lokalny i hybrydowy).
#[async_trait]
pub trait DescribePort: Send + Sync {
    /// Opisuje obraz zgodnie z prywatnością sesji.
    async fn describe(
        &self,
        request: DescribeRequest,
        cancel: CancellationToken,
    ) -> Result<Description, DescribeError>;
}

/// Limity (`[tools.vision]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisionToolsConfig {
    /// Największy plik obrazu (B).
    pub max_file_bytes: u64,
    /// Największa liczba pikseli obrazu z pliku (ochrona przed bombą dekompresyjną).
    pub max_pixels: u64,
    /// Dłuższy bok zrzutu dla OCR (64–4096).
    pub ocr_max_side: u32,
    /// Dłuższy bok zrzutu wysyłanego do modelu (64–4096).
    pub describe_max_side: u32,
    /// Największy obraz wysyłany do modelu (B).
    pub describe_max_bytes: u64,
    /// Limit tokenów opisu.
    pub describe_max_tokens: u32,
    /// Limit znaków tekstu dla modelu.
    pub output_max_chars: usize,
    /// Dodatkowe aplikacje maskowane w zrzutach (jak `[tools.screen] masked_apps`).
    pub masked_apps: Vec<String>,
}

impl Default for VisionToolsConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: 20 * 1024 * 1024,
            max_pixels: 40_000_000,
            ocr_max_side: 2600,
            describe_max_side: 1568,
            describe_max_bytes: 5 * 1024 * 1024,
            describe_max_tokens: 1024,
            output_max_chars: 20_000,
            masked_apps: Vec::new(),
        }
    }
}

/// Testy kontraktowe zestawu `tools-vision` (feature `contract-tests`).
#[cfg(feature = "contract-tests")]
pub mod contract_tests {
    use std::sync::Arc;

    use tools_common_contract::{Tool, contract_tests as common};

    use super::{manifests, sample_args};

    /// Manifesty, złe argumenty (także sprzeczne źródła), anulowanie.
    pub async fn run_all(tools: &[Arc<dyn Tool>]) {
        assert_eq!(tools.len(), manifests().len());
        for (tool, manifest) in tools.iter().zip(manifests()) {
            assert_eq!(tool.manifest(), &manifest);
            let name = manifest.name.clone();
            common::run_all(tool.as_ref(), "/", sample_args(&name)).await;
            for bad in [
                serde_json::json!({"source": "file"}),
                serde_json::json!({"source": "screen", "path": "a.png"}),
                serde_json::json!({"source": "file", "path": "a.png", "monitor": 0}),
            ] {
                assert!(
                    !tool.call(bad.clone(), &common::ctx("/")).await.is_ok(),
                    "{name}: {bad}"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests;
