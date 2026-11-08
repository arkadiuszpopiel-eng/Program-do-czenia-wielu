//! Manifest modelu słów wywoławczych (`<model>.kws.json`, obok plików ONNX): etykiety (frazy),
//! cechy wejścia, kształt wejścia, aktywacja wyjścia i **SHA-256 każdego pliku ONNX** (łańcuch
//! dostaw, PLAN §8.7 — plik o innym hashu nie jest ładowany).
//!
//! Dwa rodzaje modeli:
//! - `log_mel` — jeden klasyfikator na cechach log-mel liczonych w Rust (`voice-dsp-contract::fbank`):
//!   wejście `[1, frames, n_mels]` (`btf`), `[1, n_mels, frames]` (`bft`) albo `[1, 1, frames, n_mels]`
//!   (`b1tf`), wyjście `[1, K]` (wynik per etykieta; `sigmoid`/`softmax`/`none`), krok co `step_frames`
//!   ramek 10 ms (np. sherpa-onnx/własny trening na mowie syntetycznej + korpusie);
//! - `openwakeword` — potok openWakeWord: `melspectrogram.onnx` (`[1, 1280·k]` próbek → `[1,1,F,32]`,
//!   potem `x/10 + 2`), `embedding_model.onnx` (`[1, 76, 32, 1]` → 96 cech, co 8 ramek mel)
//!   i klasyfikator frazy (`[1, 16, 96]` → `[1, 1]`), po jednym pliku na etykietę.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use voice_wake_contract::WakeError;

/// Wersja formatu manifestu.
pub const KWS_MANIFEST_FORMAT: &str = "alfa-kws-v1";

/// Plik modelu z hashem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFile {
    /// Ścieżka względna do katalogu manifestu (bez `..`).
    pub path: String,
    /// SHA-256 (hex).
    pub sha256: String,
}

/// Aktywacja wyjścia klasyfikatora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    /// Wyjście to logity → sigmoid.
    Sigmoid,
    /// Wyjście to logity klas → softmax (z klasą tła `background_index`).
    Softmax,
    /// Wyjście to już prawdopodobieństwa.
    None,
}

/// Układ wejścia klasyfikatora `log_mel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// `[1, frames, n_mels]`.
    Btf,
    /// `[1, n_mels, frames]`.
    Bft,
    /// `[1, 1, frames, n_mels]`.
    B1tf,
}

/// Rodzaj modelu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KwsModelKind {
    /// Klasyfikator na cechach log-mel z Rust.
    LogMel {
        /// Model.
        model: ModelFile,
        /// Liczba filtrów mel.
        n_mels: usize,
        /// Ramek 10 ms w oknie wejścia.
        frames: usize,
        /// Co ile ramek liczyć wynik.
        step_frames: usize,
        /// Układ wejścia.
        layout: Layout,
        /// Aktywacja wyjścia.
        activation: Activation,
        /// Indeks klasy tła w wyjściu (pomijany w wynikach), jeśli jest.
        #[serde(default)]
        background_index: Option<usize>,
    },
    /// Potok openWakeWord (mel → embedding → klasyfikator per fraza).
    Openwakeword {
        /// `melspectrogram.onnx`.
        melspectrogram: ModelFile,
        /// `embedding_model.onnx`.
        embedding: ModelFile,
        /// Klasyfikatory w kolejności etykiet.
        classifiers: Vec<ModelFile>,
    },
}

/// Manifest modelu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KwsManifest {
    /// [`KWS_MANIFEST_FORMAT`].
    pub format: String,
    /// Nazwa (Diagnostyka).
    pub name: String,
    /// Licencja modelu (README: skąd wziąć i na jakich warunkach).
    pub license: String,
    /// Etykiety (frazy, np. „hej alfa”) w kolejności wyjść.
    pub labels: Vec<String>,
    /// Rodzaj i pliki.
    pub model: KwsModelKind,
}

fn bad(m: impl Into<String>) -> WakeError {
    WakeError::Model(m.into())
}

/// SHA-256 bajtów (hex).
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn safe_relative(path: &str) -> Result<PathBuf, WakeError> {
    let p = Path::new(path);
    let ok = !path.is_empty()
        && !path.contains(['\\', ':'])
        && p.components().all(|c| matches!(c, Component::Normal(_)));
    if ok {
        Ok(p.to_path_buf())
    } else {
        Err(bad(format!(
            "ścieżka modelu `{path}` musi być względna, bez `..`, `\\` i `:`"
        )))
    }
}

impl KwsManifest {
    /// Parsuje i sprawdza manifest.
    pub fn parse(json: &str) -> Result<Self, WakeError> {
        let m: Self = serde_json::from_str(json).map_err(|e| bad(format!("manifest KWS: {e}")))?;
        m.validate()?;
        Ok(m)
    }

    /// Spójność: format, etykiety, kształty, ścieżki.
    pub fn validate(&self) -> Result<(), WakeError> {
        if self.format != KWS_MANIFEST_FORMAT {
            return Err(bad(format!(
                "format `{}` ≠ `{KWS_MANIFEST_FORMAT}`",
                self.format
            )));
        }
        if self.labels.is_empty() || self.labels.len() > 32 {
            return Err(bad("1–32 etykiet"));
        }
        for f in self.files() {
            safe_relative(&f.path)?;
            if f.sha256.len() != 64 || !f.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(bad(format!("`{}`: sha256 musi mieć 64 znaki hex", f.path)));
            }
        }
        match &self.model {
            KwsModelKind::LogMel {
                n_mels,
                frames,
                step_frames,
                background_index,
                ..
            } => {
                if !(8..=128).contains(n_mels) || !(10..=300).contains(frames) {
                    return Err(bad("log_mel: n_mels 8–128, frames 10–300"));
                }
                if *step_frames == 0 || step_frames > frames {
                    return Err(bad("log_mel: step_frames 1…frames"));
                }
                if background_index.is_some_and(|b| b > self.labels.len()) {
                    return Err(bad("log_mel: background_index poza wyjściem"));
                }
            }
            KwsModelKind::Openwakeword { classifiers, .. } => {
                if classifiers.len() != self.labels.len() {
                    return Err(bad("openwakeword: jeden klasyfikator na etykietę"));
                }
            }
        }
        Ok(())
    }

    /// Wszystkie pliki ONNX.
    pub fn files(&self) -> Vec<&ModelFile> {
        match &self.model {
            KwsModelKind::LogMel { model, .. } => vec![model],
            KwsModelKind::Openwakeword {
                melspectrogram,
                embedding,
                classifiers,
            } => [melspectrogram, embedding]
                .into_iter()
                .chain(classifiers.iter())
                .collect(),
        }
    }
}

/// Czyta plik modelu z katalogu manifestu i sprawdza SHA-256.
pub fn read_verified(dir: &Path, file: &ModelFile) -> Result<Vec<u8>, WakeError> {
    let path = dir.join(safe_relative(&file.path)?);
    let bytes = std::fs::read(&path).map_err(|e| bad(format!("{}: {e}", path.display())))?;
    let hash = sha256_hex(&bytes);
    if !hash.eq_ignore_ascii_case(&file.sha256) {
        return Err(bad(format!(
            "{}: SHA-256 {hash} ≠ {} z manifestu — plik zmieniony albo nie ten model",
            path.display(),
            file.sha256
        )));
    }
    Ok(bytes)
}
