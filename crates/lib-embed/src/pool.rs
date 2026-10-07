//! Pooling stanów ukrytych `[wsad, sekwencja, wymiar]` do jednego wektora na tekst i normalizacja L2.

use serde::{Deserialize, Serialize};

/// Sposób redukcji stanów ukrytych do wektora tekstu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pooling {
    /// Średnia po tokenach z maską uwagi (E5, `paraphrase-multilingual-*`).
    #[default]
    Mean,
    /// Pierwszy token (`<s>`/`[CLS]`).
    Cls,
    /// Wyjście modelu jest już wektorem tekstu `[wsad, wymiar]` (np. `sentence_embedding`).
    Pooled,
}

/// Średnia stanów tokenów z `mask = 1` (wiersz `[seq * dims]`); pusta maska → wektor zerowy.
pub fn mean_pool(hidden: &[f32], mask: &[i64], dims: usize) -> Vec<f32> {
    let mut out = vec![0.0_f32; dims];
    let mut count = 0.0_f32;
    for (token, m) in hidden.chunks_exact(dims).zip(mask) {
        if *m != 0 {
            out.iter_mut().zip(token).for_each(|(o, h)| *o += h);
            count += 1.0;
        }
    }
    if count > 0.0 {
        out.iter_mut().for_each(|o| *o /= count);
    }
    out
}

/// Normalizacja L2 w miejscu; wektor zerowy (lub nieskończony) → `e0` (wynik zawsze jednostkowy).
pub fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm.is_finite() && norm > f32::EPSILON {
        v.iter_mut().for_each(|x| *x /= norm);
    } else {
        v.iter_mut().for_each(|x| *x = 0.0);
        if let Some(first) = v.first_mut() {
            *first = 1.0;
        }
    }
}

/// Wektor tekstu `row` z wyjścia modelu (`hidden` = cały tensor, `seq` = długość sekwencji wsadu).
pub fn pool_row(
    pooling: Pooling,
    hidden: &[f32],
    mask: &[i64],
    row: usize,
    seq: usize,
    dims: usize,
) -> Option<Vec<f32>> {
    match pooling {
        Pooling::Pooled => hidden
            .get(row * dims..(row + 1) * dims)
            .map(<[f32]>::to_vec),
        Pooling::Cls => hidden
            .get(row * seq * dims..row * seq * dims + dims)
            .map(<[f32]>::to_vec),
        Pooling::Mean => {
            let states = hidden.get(row * seq * dims..(row + 1) * seq * dims)?;
            let mask = mask.get(row * seq..(row + 1) * seq)?;
            Some(mean_pool(states, mask, dims))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_respects_mask_and_cls_takes_first() {
        // 2 teksty × 3 tokeny × 2 wymiary; drugi tekst ma 1 token wypełnienia.
        let hidden = [
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 1.0, 1.0, 3.0, 3.0, 100.0, 100.0,
        ];
        let mask = [1, 1, 1, 1, 1, 0];
        assert_eq!(
            pool_row(Pooling::Mean, &hidden, &mask, 0, 3, 2).unwrap(),
            [3.0, 4.0]
        );
        assert_eq!(
            pool_row(Pooling::Mean, &hidden, &mask, 1, 3, 2).unwrap(),
            [2.0, 2.0]
        );
        assert_eq!(
            pool_row(Pooling::Cls, &hidden, &mask, 1, 3, 2).unwrap(),
            [1.0, 1.0]
        );
        assert_eq!(
            pool_row(Pooling::Pooled, &hidden, &mask, 1, 3, 2).unwrap(),
            [3.0, 4.0]
        );
        assert!(pool_row(Pooling::Mean, &hidden, &mask, 2, 3, 2).is_none());
        assert_eq!(mean_pool(&[1.0, 2.0], &[0], 2), [0.0, 0.0]);
    }

    #[test]
    fn normalization_is_unit_or_e0() {
        let mut v = [3.0, 4.0];
        l2_normalize(&mut v);
        assert_eq!(v, [0.6, 0.8]);
        let mut z = [0.0, 0.0, 0.0];
        l2_normalize(&mut z);
        assert_eq!(z, [1.0, 0.0, 0.0]);
        let mut n = [f32::NAN, 1.0];
        l2_normalize(&mut n);
        assert_eq!(n, [1.0, 0.0]);
        let mut empty: [f32; 0] = [];
        l2_normalize(&mut empty);
    }
}
