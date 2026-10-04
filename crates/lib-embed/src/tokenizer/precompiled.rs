//! Normalizator `Precompiled` (SentencePiece `precompiled_charsmap`, np. `nmt_nfkc` w XLM-R):
//! podwójna tablica (Darts) z regułami zamiany + pula znormalizowanych napisów zakończonych `\0`.
//!
//! Semantyka jak w HF `tokenizers` (`spm_precompiled`): tekst dzielony na klastry grafemów; klaster
//! krótszy niż 6 bajtów zamieniany w całości, jeśli ma regułę; inaczej każdy znak osobno. Reguła =
//! **pierwsze** (najkrótsze) dopasowanie prefiksu w tablicy.

use unicode_segmentation::UnicodeSegmentation;

use crate::error::EmbedError;

fn bad(reason: impl Into<String>) -> EmbedError {
    EmbedError::Tokenizer(format!("precompiled_charsmap: {}", reason.into()))
}

/// Dekoder Base64 (alfabet standardowy, `=` na końcu opcjonalne).
pub fn base64_decode(text: &str) -> Result<Vec<u8>, EmbedError> {
    fn value(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let body = text.trim_end_matches('=').as_bytes();
    if body.len() % 4 == 1 {
        return Err(bad("niepoprawna długość Base64"));
    }
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    for chunk in body.chunks(4) {
        let mut acc = 0_u32;
        for (i, c) in chunk.iter().enumerate() {
            let v = value(*c).ok_or_else(|| bad("znak spoza Base64"))?;
            acc |= v << (18 - 6 * i);
        }
        let bytes = acc.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    Ok(out)
}

/// Reguły normalizacji z `precompiled_charsmap`.
#[derive(Debug, Clone)]
pub struct Precompiled {
    trie: Vec<u32>,
    normalized: Vec<u8>,
}

fn has_leaf(unit: u32) -> bool {
    (unit >> 8) & 1 == 1
}

fn value(unit: u32) -> u32 {
    unit & ((1_u32 << 31) - 1)
}

fn label(unit: u32) -> u32 {
    unit & ((1_u32 << 31) | 0xFF)
}

fn offset(unit: u32) -> usize {
    ((unit >> 10) << ((unit & (1_u32 << 9)) >> 6)) as usize
}

impl Precompiled {
    /// Parsuje blob: `u32` LE = rozmiar tablicy w bajtach, tablica `u32` LE, pula napisów.
    pub fn from_bytes(blob: &[u8]) -> Result<Self, EmbedError> {
        let size_bytes = blob
            .get(..4)
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map(u32::from_le_bytes)
            .ok_or_else(|| bad("za krótki"))? as usize;
        let trie_bytes = blob
            .get(4..4 + size_bytes)
            .filter(|b| b.len() % 4 == 0 && !b.is_empty())
            .ok_or_else(|| bad("rozmiar tablicy poza blobem"))?;
        let trie = trie_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let normalized = blob.get(4 + size_bytes..).unwrap_or_default().to_vec();
        if std::str::from_utf8(&normalized).is_err() {
            return Err(bad("pula napisów nie jest UTF-8"));
        }
        Ok(Self { trie, normalized })
    }

    /// Z tekstu Base64 (pole `precompiled_charsmap` w `tokenizer.json`).
    pub fn from_base64(text: &str) -> Result<Self, EmbedError> {
        Self::from_bytes(&base64_decode(text)?)
    }

    /// Pierwsza (najkrótsza) wartość prefiksu `key` w tablicy; `None` przy braku albo błędnych danych.
    fn first_prefix_value(&self, key: &[u8]) -> Option<u32> {
        let mut node = 0_usize;
        let mut unit = *self.trie.get(node)?;
        node ^= offset(unit);
        for &c in key {
            if c == 0 {
                return None;
            }
            node ^= usize::from(c);
            unit = *self.trie.get(node)?;
            if label(unit) != u32::from(c) {
                return None;
            }
            node ^= offset(unit);
            if has_leaf(unit) {
                return self.trie.get(node).map(|u| value(*u));
            }
        }
        None
    }

    /// Zamiana fragmentu wg reguły (napis z puli do pierwszego `\0`).
    pub fn transform(&self, chunk: &str) -> Option<&str> {
        let start = usize::try_from(self.first_prefix_value(chunk.as_bytes())?).ok()?;
        let tail = self.normalized.get(start..)?;
        let end = tail.iter().position(|b| *b == 0).unwrap_or(tail.len());
        std::str::from_utf8(&tail[..end]).ok()
    }

    /// Normalizuje tekst (grafemy < 6 bajtów w całości, inaczej znak po znaku).
    pub fn normalize(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for grapheme in text.graphemes(true) {
            if grapheme.len() < 6
                && let Some(norm) = self.transform(grapheme)
            {
                out.push_str(norm);
                continue;
            }
            for (i, c) in grapheme.char_indices() {
                match self.transform(&grapheme[i..i + c.len_utf8()]) {
                    Some(norm) => out.push_str(norm),
                    None => out.push(c),
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip_and_errors() {
        assert_eq!(base64_decode("TWFu").unwrap(), b"Man");
        assert_eq!(base64_decode("TWE=").unwrap(), b"Ma");
        assert_eq!(base64_decode("TQ==").unwrap(), b"M");
        assert_eq!(base64_decode("").unwrap(), b"");
        assert!(base64_decode("T").is_err());
        assert!(base64_decode("T$==").is_err());
    }

    #[test]
    fn malformed_blob_is_rejected_without_panic() {
        assert!(Precompiled::from_bytes(&[]).is_err());
        assert!(Precompiled::from_bytes(&[8, 0, 0, 0, 1]).is_err());
        assert!(Precompiled::from_bytes(&[4, 0, 0, 0, 0, 0, 0, 0, 0xFF]).is_err());
        // Tablica z jednym węzłem bez dzieci: brak reguł, tekst bez zmian.
        let p = Precompiled::from_bytes(&[4, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(p.normalize("Zażółć"), "Zażółć");
        assert_eq!(p.transform("a"), None);
    }
}
