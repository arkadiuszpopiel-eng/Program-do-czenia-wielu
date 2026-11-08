//! Embedding mówcy z modelu ONNX (ECAPA-TDNN / WeSpeaker ResNet / 3D-Speaker — np. modele
//! sherpa-onnx „speaker recognition”) przez `tract-onnx` (czysty Rust, ADR 0004).
//!
//! Manifest `<model>.speaker.json` (`alfa-speaker-v1`): plik ONNX z **SHA-256** (inny plik nie jest
//! ładowany), cechy Kaldi fbank (`n_mels`, domyślnie 80, skala int16), CMN po wypowiedzi, okno
//! `frames` ramek 10 ms (stały kształt `[1, frames, n_mels]` dla `tract`), przesunięcie okna
//! `hop_frames`; wypowiedź krótsza niż okno jest powielana (zawijanie cech). Embedding = średnia
//! znormalizowanych embeddingów okien.

use std::path::{Component, Path};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tract_onnx::prelude::*;
use voice_dsp_contract::fbank::cmn;
use voice_dsp_contract::{Fbank, FbankCfg};
use voice_speaker_contract::{Embedding, EmbeddingModel, SpeakerError, mean_normalized};

/// Wersja formatu manifestu.
pub const SPEAKER_MANIFEST_FORMAT: &str = "alfa-speaker-v1";

/// Manifest modelu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeakerManifest {
    /// [`SPEAKER_MANIFEST_FORMAT`].
    pub format: String,
    /// Identyfikator modelu (zapisywany w profilu).
    pub name: String,
    /// Licencja modelu.
    pub license: String,
    /// Plik ONNX (względnie do manifestu).
    pub path: String,
    /// SHA-256 pliku.
    pub sha256: String,
    /// Filtry mel.
    pub n_mels: usize,
    /// Ramek w oknie.
    pub frames: usize,
    /// Przesunięcie okna (ramki).
    pub hop_frames: usize,
    /// Normalizacja średniej po wypowiedzi.
    #[serde(default = "yes")]
    pub cmn: bool,
}

fn yes() -> bool {
    true
}

fn err(e: impl std::fmt::Debug) -> SpeakerError {
    SpeakerError::Model(format!("{e:?}"))
}

/// SHA-256 (hex).
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl SpeakerManifest {
    /// Parsuje i sprawdza manifest.
    pub fn parse(json: &str) -> Result<Self, SpeakerError> {
        let m: Self = serde_json::from_str(json).map_err(|e| err(e.to_string()))?;
        let rel_ok = !m.path.contains(['\\', ':'])
            && Path::new(&m.path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)));
        let ok = m.format == SPEAKER_MANIFEST_FORMAT
            && !m.name.trim().is_empty()
            && rel_ok
            && m.sha256.len() == 64
            && (20..=128).contains(&m.n_mels)
            && (50..=1_000).contains(&m.frames)
            && m.hop_frames > 0
            && m.hop_frames <= m.frames;
        if ok {
            Ok(m)
        } else {
            Err(SpeakerError::Model(
                "manifest mówcy: format alfa-speaker-v1, ścieżka względna bez `..`, sha256 (64 hex), \
                 n_mels 20–128, frames 50–1000, 0 < hop ≤ frames"
                    .into(),
            ))
        }
    }
}

/// Model ONNX embeddingu mówcy.
pub struct OnnxSpeakerModel {
    name: String,
    plan: Arc<TypedRunnableModel>,
    fbank: Fbank,
    frames: usize,
    hop: usize,
    n_mels: usize,
    cmn: bool,
}

impl std::fmt::Debug for OnnxSpeakerModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxSpeakerModel")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl OnnxSpeakerModel {
    /// Ładuje model z manifestu (sprawdza SHA-256).
    pub fn load(manifest_path: &Path) -> Result<Self, SpeakerError> {
        let json = std::fs::read_to_string(manifest_path)
            .map_err(|e| SpeakerError::Model(format!("{}: {e}", manifest_path.display())))?;
        let m = SpeakerManifest::parse(&json)?;
        let dir = manifest_path.parent().unwrap_or(Path::new("."));
        let bytes = std::fs::read(dir.join(&m.path))
            .map_err(|e| SpeakerError::Model(format!("{}: {e}", m.path)))?;
        let hash = sha256_hex(&bytes);
        if !hash.eq_ignore_ascii_case(&m.sha256) {
            return Err(SpeakerError::Model(format!(
                "{}: SHA-256 {hash} ≠ {} z manifestu",
                m.path, m.sha256
            )));
        }
        Self::from_bytes(&m, &bytes)
    }

    /// Model z bajtów (już zweryfikowanych).
    pub fn from_bytes(m: &SpeakerManifest, bytes: &[u8]) -> Result<Self, SpeakerError> {
        let plan = tract_onnx::onnx()
            .model_for_read(&mut &bytes[..])
            .map_err(err)?
            .with_input_fact(0, f32::fact([1, m.frames, m.n_mels]).into())
            .map_err(err)?
            .into_optimized()
            .map_err(err)?
            .into_runnable()
            .map_err(err)?;
        Ok(Self {
            name: m.name.clone(),
            plan,
            fbank: Fbank::new(FbankCfg::kaldi(m.n_mels)).map_err(err)?,
            frames: m.frames,
            hop: m.hop_frames,
            n_mels: m.n_mels,
            cmn: m.cmn,
        })
    }

    fn window(&self, feats: &[Vec<f32>]) -> Result<Embedding, SpeakerError> {
        let data: Vec<f32> = feats.iter().flatten().copied().collect();
        let x = Tensor::from_shape(&[1, self.frames, self.n_mels], &data).map_err(err)?;
        let out = self.plan.run(tvec!(x.into_tvalue())).map_err(err)?;
        let first = out.first().ok_or_else(|| err("brak wyjścia"))?;
        let view = first.to_plain_array_view::<f32>().map_err(err)?;
        Embedding::new(view.iter().copied().collect())
    }
}

impl EmbeddingModel for OnnxSpeakerModel {
    fn model_id(&self) -> &str {
        &self.name
    }

    fn embed(&mut self, audio: &[f32]) -> Result<Embedding, SpeakerError> {
        let mut feats = self.fbank.compute(audio);
        if feats.is_empty() {
            return Err(SpeakerError::TooShort { ms: 0, min_ms: 25 });
        }
        if self.cmn {
            cmn(&mut feats);
        }
        let n = feats.len();
        let mut windows = Vec::new();
        let mut start = 0;
        loop {
            let w: Vec<Vec<f32>> = (0..self.frames)
                .map(|i| feats[(start + i) % n].clone())
                .collect();
            windows.push(self.window(&w)?);
            start += self.hop;
            if start + self.frames > n {
                break;
            }
        }
        mean_normalized(&windows)
    }
}
