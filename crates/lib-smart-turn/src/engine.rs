//! Model Smart Turn na ONNX Runtime (procesor): cechy log-mel → jedno prawdopodobieństwo.

use std::path::Path;

use ort::session::Session;
use ort::value::Tensor;

use crate::mel::{MelExtractor, N_FRAMES, N_MELS};
use crate::protocol::Scorer;

/// Błąd silnika (tekst trafia do odpowiedzi `error`).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Brak pliku modelu.
    #[error("brak pliku: {0}")]
    Missing(String),
    /// Model ma inny układ wejść albo wyjść, niż oczekiwany.
    #[error("nieoczekiwany układ modelu: {0}")]
    Layout(String),
    /// Błąd ONNX Runtime.
    #[error("ONNX Runtime: {0}")]
    Runtime(String),
}

impl From<ort::Error> for EngineError {
    fn from(e: ort::Error) -> Self {
        Self::Runtime(e.to_string())
    }
}

/// Wczytany model.
pub struct SmartTurn {
    session: Session,
    input: String,
    output: String,
    mel: MelExtractor,
}

impl std::fmt::Debug for SmartTurn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmartTurn")
            .field("input", &self.input)
            .field("output", &self.output)
            .finish_non_exhaustive()
    }
}

impl SmartTurn {
    /// Wczytuje model z pliku (`threads` wątków obliczeń).
    pub fn load(path: &Path, threads: usize) -> Result<Self, EngineError> {
        if !path.is_file() {
            return Err(EngineError::Missing(path.display().to_string()));
        }
        let session = Session::builder()?
            .with_intra_threads(threads.clamp(1, 16))?
            .commit_from_file(path)?;
        let input = session
            .inputs
            .first()
            .map(|i| i.name.clone())
            .ok_or_else(|| EngineError::Layout("brak wejścia".into()))?;
        let output = session
            .outputs
            .first()
            .map(|o| o.name.clone())
            .ok_or_else(|| EngineError::Layout("brak wyjścia".into()))?;
        Ok(Self {
            session,
            input,
            output,
            mel: MelExtractor::new(),
        })
    }

    /// Prawdopodobieństwo końca wypowiedzi dla nagrania 16 kHz mono.
    pub fn probability(&mut self, samples: &[f32]) -> Result<f32, EngineError> {
        let feats = self.mel.log_mel(samples);
        let tensor = Tensor::from_array(([1usize, N_MELS, N_FRAMES], feats))?;
        let out = self
            .session
            .run(ort::inputs![self.input.as_str() => &tensor])?;
        let (_, data) = out[self.output.as_str()].try_extract_tensor::<f32>()?;
        let raw = data
            .first()
            .copied()
            .ok_or_else(|| EngineError::Layout("puste wyjście".into()))?;
        // Głowica modelu zwraca już prawdopodobieństwo; surową wartość spoza 0–1 przeliczamy.
        Ok(if (0.0..=1.0).contains(&raw) {
            raw
        } else {
            1.0 / (1.0 + (-raw).exp())
        })
    }
}

impl Scorer for SmartTurn {
    fn score(&mut self, samples: &[f32]) -> Result<f32, String> {
        if samples.is_empty() {
            return Err("puste nagranie".into());
        }
        self.probability(samples).map_err(|e| e.to_string())
    }
}
