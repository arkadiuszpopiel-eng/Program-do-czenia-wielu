//! Modele słów wywoławczych przez `tract-onnx`: manifest z hashami ([`KwsManifest`]),
//! klasyfikator log-mel i potok openWakeWord; [`load_scorer`] → `Box<dyn KeywordScorer>`.
//! Model **nie jest w repo** (README: skąd wziąć, licencje, trening frazy PL).

mod logmel;
pub mod manifest;
mod onnx;
mod oww;

use std::path::Path;

use voice_wake_contract::{KeywordScorer, WakeError};

pub use manifest::{
    Activation, KWS_MANIFEST_FORMAT, KwsManifest, KwsModelKind, Layout, ModelFile, read_verified,
    sha256_hex,
};

/// Ładuje model z manifestu (`<katalog>/<nazwa>.kws.json`); każdy plik ONNX musi mieć hash
/// z manifestu.
pub fn load_scorer(manifest_path: &Path) -> Result<Box<dyn KeywordScorer>, WakeError> {
    let json = std::fs::read_to_string(manifest_path)
        .map_err(|e| WakeError::Model(format!("{}: {e}", manifest_path.display())))?;
    let manifest = KwsManifest::parse(&json)?;
    let dir = manifest_path.parent().unwrap_or(Path::new("."));
    scorer_from_manifest(&manifest, dir)
}

/// Model z gotowego manifestu i katalogu plików.
pub fn scorer_from_manifest(
    manifest: &KwsManifest,
    dir: &Path,
) -> Result<Box<dyn KeywordScorer>, WakeError> {
    let labels = manifest.labels.clone();
    match &manifest.model {
        KwsModelKind::LogMel {
            model,
            n_mels,
            frames,
            step_frames,
            layout,
            activation,
            background_index,
        } => {
            let bytes = read_verified(dir, model)?;
            let spec = logmel::LogMelSpec {
                n_mels: *n_mels,
                frames: *frames,
                step_frames: *step_frames,
                layout: *layout,
                activation: *activation,
                background_index: *background_index,
            };
            Ok(Box::new(logmel::LogMelScorer::new(labels, spec, &bytes)?))
        }
        KwsModelKind::Openwakeword {
            melspectrogram,
            embedding,
            classifiers,
        } => {
            let mel = read_verified(dir, melspectrogram)?;
            let emb = read_verified(dir, embedding)?;
            let cls = classifiers
                .iter()
                .map(|c| read_verified(dir, c))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Box::new(oww::OwwScorer::new(labels, &mel, &emb, &cls)?))
        }
    }
}
