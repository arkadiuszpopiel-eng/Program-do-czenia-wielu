//! Lokalny osadzacz leksykalny (hashing trick na słowach złożonych `fold_pl`): lekki,
//! deterministyczny, bez modelu ML. Zapewnia działanie trybu wektorowego/hybrydowego `search`
//! do czasu podłączenia osadzeń ONNX (ADR 0004) — wymiana = inny `model_id` (reindeks).

use search_contract::{Embedder, SearchError};

/// Identyfikator modelu zapisywany w indeksie.
pub const LEXICAL_MODEL_ID: &str = "alfa-lexical-hash-v1";
/// Wymiar wektora.
pub const LEXICAL_DIMS: usize = 256;

/// Osadzacz leksykalny.
#[derive(Debug, Default, Clone, Copy)]
pub struct LexicalEmbedder;

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn embed_one(text: &str) -> Vec<f32> {
    let mut v = vec![0.0_f32; LEXICAL_DIMS];
    let tokens = lib_sqlstore::search_tokens(text);
    let dims = LEXICAL_DIMS as u64;
    let mut add = |feature: &str, weight: f32| {
        let h = fnv1a(feature.as_bytes());
        let bucket = usize::try_from(h % dims).unwrap_or(0);
        let sign = if h >> 63 == 0 { 1.0 } else { -1.0 };
        v[bucket] += sign * weight;
    };
    for token in &tokens {
        add(token, 1.0);
    }
    for pair in tokens.windows(2) {
        add(&format!("{} {}", pair[0], pair[1]), 0.5);
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    } else {
        v[0] = 1.0;
    }
    v
}

impl Embedder for LexicalEmbedder {
    fn model_id(&self) -> &str {
        LEXICAL_MODEL_ID
    }

    fn dims(&self) -> usize {
        LEXICAL_DIMS
    }

    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        Ok(texts.iter().map(|t| embed_one(t)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cos(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    #[test]
    fn deterministic_normalized_and_diacritics_insensitive() {
        let e = LexicalEmbedder;
        let out = e
            .embed(&["Żółta łódź", "zolta lodz", "raport kwartalny", ""])
            .unwrap();
        assert_eq!(out.len(), 4);
        assert!(out.iter().all(|v| v.len() == LEXICAL_DIMS));
        assert!((cos(&out[0], &out[0]) - 1.0).abs() < 1e-5);
        assert!(cos(&out[0], &out[1]) > 0.99, "fold_pl: bez diakrytyków");
        assert!(cos(&out[0], &out[2]) < 0.5);
        assert!((cos(&out[3], &out[3]) - 1.0).abs() < 1e-5);
        assert_eq!(e.embed(&["x"]).unwrap(), e.embed(&["x"]).unwrap());
    }
}
