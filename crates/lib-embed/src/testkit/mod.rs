//! Zestaw testowy (feature `testkit`): zabawkowe enkodery ONNX (wagi deterministyczne; także
//! o budowie eksportu HF XLM-R — [`xlmr_like_model_bytes`]) + tokenizer
//! Unigram (2002 kawałki, wytrenowany na polskich tekstach repo) + manifest `alfa-embed-v1`.
//! Przechodzi całą ścieżkę produkcyjną (SHA-256, tokenizer, `tract`, pooling) bez pobierania modeli —
//! dla testów `lib-embed` i ścieżki „z atrapą” evalu F7-02 w `memory-impl`.

mod onnx;
mod xlmr;

use std::path::{Path, PathBuf};

pub use onnx::{TOY_MAX_POS, toy_model_bytes, weights};
pub use xlmr::{
    XLMR_DIMS, XLMR_FFN, XLMR_HEADS, XLMR_MAX_POS, XlmrShape, xlmr_like_model_bytes,
    xlmr_model_bytes,
};

use crate::error::EmbedError;
use crate::manifest::{EMBED_MANIFEST_FORMAT, EmbedManifest, FileRef, MANIFEST_FILE, sha256_hex};
use crate::pool::Pooling;

/// Tokenizer zabawkowy (`tokenizer.json`: NFKC + Lowercase, `Metaspace`, `<s> $A </s>`).
pub const TOY_TOKENIZER_JSON: &str = include_str!("toy-tokenizer.json");

/// Rozmiar słownika tokenizera zabawkowego.
pub const TOY_VOCAB: usize = 2002;

/// Wymiar wektora zabawkowego modelu.
pub const TOY_DIMS: usize = 32;

/// Manifest zabawkowego modelu dla podanych bajtów (prefiksy E5, pooling średni, wsad 4).
pub fn toy_manifest(model: &[u8], tokenizer: &[u8]) -> EmbedManifest {
    EmbedManifest {
        format: EMBED_MANIFEST_FORMAT.into(),
        id: "toy-encoder".into(),
        license: "CC0-1.0".into(),
        model: FileRef {
            path: "model.onnx".into(),
            sha256: sha256_hex(model),
        },
        tokenizer: FileRef {
            path: "tokenizer.json".into(),
            sha256: sha256_hex(tokenizer),
        },
        dims: TOY_DIMS,
        max_tokens: TOY_MAX_POS,
        pooling: Pooling::Mean,
        normalize: true,
        query_prefix: "query: ".into(),
        passage_prefix: "passage: ".into(),
        output: None,
        batch_size: 4,
        ram_mb: 8,
        idle_unload_s: 300,
    }
}

/// Zapisuje model, tokenizer i manifest w `dir`; zwraca ścieżkę manifestu.
pub fn write_toy_model(dir: &Path) -> Result<PathBuf, EmbedError> {
    let model = toy_model_bytes(TOY_VOCAB, TOY_DIMS, false);
    let manifest = toy_manifest(&model, TOY_TOKENIZER_JSON.as_bytes());
    write_files(dir, &manifest, &model, TOY_TOKENIZER_JSON.as_bytes())
}

/// Zapisuje pliki modelu wg manifestu (ścieżki z manifestu) i sam manifest.
pub fn write_files(
    dir: &Path,
    manifest: &EmbedManifest,
    model: &[u8],
    tokenizer: &[u8],
) -> Result<PathBuf, EmbedError> {
    std::fs::create_dir_all(dir).map_err(|e| EmbedError::io(dir, e))?;
    for (file, bytes) in [(&manifest.model, model), (&manifest.tokenizer, tokenizer)] {
        let path = dir.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| EmbedError::io(parent, e))?;
        }
        std::fs::write(&path, bytes).map_err(|e| EmbedError::io(&path, e))?;
    }
    let path = dir.join(MANIFEST_FILE);
    std::fs::write(&path, manifest.to_json()).map_err(|e| EmbedError::io(&path, e))?;
    Ok(path)
}
