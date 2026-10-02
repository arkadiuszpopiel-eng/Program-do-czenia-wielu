//! Embedding mówcy (wektor znormalizowany L2) i model, który go liczy (ECAPA/WeSpeaker przez
//! `tract-onnx` w `-impl`, deterministyczny model cech w `-fake`).

use crate::SpeakerError;

/// Embedding (L2 = 1; pusty/zerowy wektor jest odrzucany).
#[derive(Clone, PartialEq)]
pub struct Embedding(Vec<f32>);

impl std::fmt::Debug for Embedding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Embedding(<{} wymiarów>)", self.0.len())
    }
}

impl Embedding {
    /// Normalizuje wektor; błąd dla pustego, zerowego albo z NaN.
    pub fn new(v: Vec<f32>) -> Result<Self, SpeakerError> {
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if v.is_empty() || !norm.is_finite() || norm <= f32::EPSILON {
            return Err(SpeakerError::Model("pusty albo zerowy embedding".into()));
        }
        Ok(Self(v.into_iter().map(|x| x / norm).collect()))
    }

    /// Wartości.
    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }

    /// Wymiar.
    pub fn dim(&self) -> usize {
        self.0.len()
    }

    /// Wektor (np. do zapisu zaszyfrowanego).
    pub fn into_vec(self) -> Vec<f32> {
        self.0
    }

    /// Zeruje wartości (porzucona rejestracja, usunięcie).
    pub fn wipe(&mut self) {
        self.0.iter_mut().for_each(|x| *x = 0.0);
    }
}

/// Kosinus dwóch embeddingów (różne wymiary → `NaN`).
pub fn cosine(a: &Embedding, b: &Embedding) -> f32 {
    if a.dim() != b.dim() {
        return f32::NAN;
    }
    a.0.iter().zip(&b.0).map(|(x, y)| x * y).sum()
}

/// Znormalizowana średnia embeddingów (profil).
pub fn mean_normalized(items: &[Embedding]) -> Result<Embedding, SpeakerError> {
    let dim = items
        .first()
        .map(Embedding::dim)
        .ok_or_else(|| SpeakerError::Model("brak embeddingów".into()))?;
    if items.iter().any(|e| e.dim() != dim) {
        return Err(SpeakerError::Model("różne wymiary embeddingów".into()));
    }
    let mut sum = vec![0.0f32; dim];
    for e in items {
        for (s, x) in sum.iter_mut().zip(&e.0) {
            *s += x;
        }
    }
    Embedding::new(sum)
}

/// Model embeddingu mówcy.
pub trait EmbeddingModel: Send {
    /// Identyfikator modelu (profil zapisany innym modelem jest nieważny).
    fn model_id(&self) -> &str;
    /// Embedding wypowiedzi 16 kHz mono.
    fn embed(&mut self, audio: &[f32]) -> Result<Embedding, SpeakerError>;
}
