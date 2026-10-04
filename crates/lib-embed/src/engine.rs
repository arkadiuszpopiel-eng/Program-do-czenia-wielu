//! Silnik embeddingu: tokenizer + model + pooling. Wsady po `batch_size` tekstów o podobnej długości
//! (sortowanie po liczbie tokenów — mniej wypełnienia), wynik w kolejności wejścia, wektory L2.

use std::path::Path;

use crate::error::EmbedError;
use crate::manifest::EmbedManifest;
use crate::model::OnnxModel;
use crate::pool::{Pooling, l2_normalize, pool_row};
use crate::tokenizer::TextTokenizer;

/// Załadowany model z tokenizerem.
#[derive(Debug)]
pub struct Engine {
    manifest: EmbedManifest,
    tokenizer: TextTokenizer,
    model: OnnxModel,
}

impl Engine {
    /// Ładuje pliki z katalogu manifestu (każdy po sprawdzeniu SHA-256).
    pub fn load(manifest: &EmbedManifest, dir: &Path) -> Result<Self, EmbedError> {
        manifest.validate()?;
        let tok = EmbedManifest::read_verified(dir, &manifest.tokenizer)?;
        let tok = String::from_utf8(tok)
            .map_err(|_| EmbedError::Tokenizer("tokenizer.json nie jest UTF-8".into()))?;
        let tokenizer = TextTokenizer::from_json(&tok)?;
        drop(tok);
        let bytes = EmbedManifest::read_verified(dir, &manifest.model)?;
        let model = OnnxModel::from_bytes(bytes, manifest.output.as_deref())?;
        Ok(Self {
            manifest: manifest.clone(),
            tokenizer,
            model,
        })
    }

    /// Manifest.
    pub fn manifest(&self) -> &EmbedManifest {
        &self.manifest
    }

    /// Identyfikatory tokenów tekstu (z tokenami specjalnymi, po obcięciu).
    pub fn tokenize(&self, text: &str) -> Vec<u32> {
        self.tokenizer.encode(text, self.manifest.max_tokens)
    }

    /// Wektory tekstów (prefiksy dokleja wywołujący).
    pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let encoded: Vec<Vec<u32>> = texts.iter().map(|t| self.tokenize(t)).collect();
        let mut order: Vec<usize> = (0..texts.len()).collect();
        order.sort_by_key(|i| (encoded[*i].len(), *i));
        let mut out: Vec<Vec<f32>> = vec![Vec::new(); texts.len()];
        for chunk in order.chunks(self.manifest.batch_size) {
            let seq = chunk
                .iter()
                .map(|i| encoded[*i].len())
                .max()
                .unwrap_or(1)
                .max(1);
            let pad = i64::from(self.tokenizer.pad_id());
            let mut ids = Vec::with_capacity(chunk.len() * seq);
            let mut mask = Vec::with_capacity(chunk.len() * seq);
            for i in chunk {
                let row = &encoded[*i];
                ids.extend(row.iter().map(|t| i64::from(*t)));
                ids.extend(std::iter::repeat_n(pad, seq - row.len()));
                mask.extend(std::iter::repeat_n(1_i64, row.len()));
                mask.extend(std::iter::repeat_n(0_i64, seq - row.len()));
            }
            let output = self.model.run(&ids, &mask, chunk.len(), seq)?;
            self.check_shape(&output.shape, chunk.len(), seq)?;
            for (row, i) in chunk.iter().enumerate() {
                let mut v = pool_row(
                    self.manifest.pooling,
                    &output.data,
                    &mask,
                    row,
                    seq,
                    self.manifest.dims,
                )
                .ok_or_else(|| EmbedError::Model("wyjście krótsze niż wsad".into()))?;
                l2_normalize(&mut v);
                out[*i] = v;
            }
        }
        Ok(out)
    }

    fn check_shape(&self, shape: &[usize], batch: usize, seq: usize) -> Result<(), EmbedError> {
        let dims = self.manifest.dims;
        let expected: Vec<usize> = match self.manifest.pooling {
            Pooling::Pooled => vec![batch, dims],
            Pooling::Mean | Pooling::Cls => vec![batch, seq, dims],
        };
        if shape == expected.as_slice() {
            Ok(())
        } else {
            Err(EmbedError::Model(format!(
                "kształt wyjścia {shape:?} ≠ {expected:?} (manifest: dims {dims}, pooling {:?})",
                self.manifest.pooling
            )))
        }
    }
}
