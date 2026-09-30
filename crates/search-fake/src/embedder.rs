//! Deterministyczny embedder atrapy: hash n-gramów znakowych → wektor 64-wymiarowy.

use lib_sqlstore::search_tokens;
use search_contract::{Embedder, SearchError};

/// Wymiar wektora atrapy.
pub const HASH_DIMS: usize = 64;

/// Embedder bez ML: dla każdego słowa (złożonego `fold_pl`, małe litery) trygramy `#słowo#` i całe
/// słowo → FNV-1a 64 → indeks `h % 64`, znak z najwyższego bitu; wektor znormalizowany L2.
/// Teksty o wspólnych słowach/rdzeniach są bliskie; ten sam tekst → ten sam wektor.
#[derive(Debug, Clone, Copy, Default)]
pub struct HashEmbedder;

impl HashEmbedder {
    /// Nowy embedder.
    pub fn new() -> Self {
        Self
    }

    /// Wektor dla jednego tekstu (tekst bez słów → wektor jednostkowy `e0`).
    pub fn vector(text: &str) -> Vec<f32> {
        let mut v = vec![0.0_f32; HASH_DIMS];
        for term in search_tokens(text) {
            let padded: Vec<char> = format!("#{term}#").chars().collect();
            let grams = padded.windows(3).map(|w| w.iter().collect::<String>());
            for gram in grams.chain(std::iter::once(term.clone())) {
                let h = fnv1a(gram.as_bytes());
                let idx = usize::try_from(h % HASH_DIMS as u64).unwrap_or_default();
                v[idx] += if h >> 63 == 0 { 1.0 } else { -1.0 };
            }
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm == 0.0 {
            v[0] = 1.0;
        } else {
            v.iter_mut().for_each(|x| *x /= norm);
        }
        v
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl Embedder for HashEmbedder {
    fn model_id(&self) -> &str {
        "fake-hash-ngram-64"
    }

    fn dims(&self) -> usize {
        HASH_DIMS
    }

    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        Ok(texts.iter().map(|t| Self::vector(t)).collect())
    }
}

/// Podobieństwo kosinusowe wektorów znormalizowanych (iloczyn skalarny).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_normalized_and_similar_for_shared_words() {
        let a = HashEmbedder::vector("Żółta łódź płynie");
        assert_eq!(a, HashEmbedder::vector("zolta lodz plynie"));
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-5);
        let near = HashEmbedder::vector("łódź płynie szybko");
        let far = HashEmbedder::vector("kalkulator podatkowy");
        assert!(cosine(&a, &near) > cosine(&a, &far));
        assert_eq!(HashEmbedder::vector(" ;; ")[0], 1.0);
        assert_eq!(HashEmbedder.embed(&["a", "b"]).unwrap().len(), 2);
    }
}
