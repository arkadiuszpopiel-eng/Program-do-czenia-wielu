//! Cache fraz stałych na dysku: klucz = SHA-256(tekst + brzmienie + tempo), plik `<klucz>.phrase`
//! (nagłówek JSON + próbki `f32` LE), limit rozmiaru z wyrzucaniem najstarszych (LRU po czasie zapisu).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use voice_tts_contract::{MarksKind, WordMark};

/// Zapisana fraza.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedPhrase {
    /// Znaczniki słów.
    pub marks: Vec<WordMark>,
    /// Rodzaj znaczników.
    pub kind: MarksKind,
    /// Silnik źródłowy.
    pub engine: String,
    /// Próbki (mono, `TTS_RATE`) — poza nagłówkiem.
    #[serde(skip)]
    pub pcm: Vec<f32>,
}

/// Cache fraz.
#[derive(Debug, Clone)]
pub struct PhraseCache {
    dir: PathBuf,
    max_bytes: u64,
}

impl PhraseCache {
    /// Cache w katalogu `dir` (tworzony) z limitem `max_bytes` (SPEC: ≤ 50 MB).
    pub fn new(dir: PathBuf, max_bytes: u64) -> std::io::Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Self { dir, max_bytes })
    }

    /// Klucz frazy.
    pub fn key(text: &str, timbre: &str, rate: f32) -> String {
        let mut h = Sha256::new();
        h.update(text.trim().as_bytes());
        h.update([0]);
        h.update(timbre.as_bytes());
        h.update(rate.to_le_bytes());
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.phrase"))
    }

    /// Odczyt (uszkodzony plik = brak).
    pub fn get(&self, key: &str) -> Option<CachedPhrase> {
        let bytes = fs::read(self.path(key)).ok()?;
        let len = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
        let mut phrase: CachedPhrase = serde_json::from_slice(bytes.get(4..4 + len)?).ok()?;
        phrase.pcm = bytes
            .get(4 + len..)?
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Some(phrase)
    }

    /// Zapis + przycięcie do limitu.
    pub fn put(&self, key: &str, phrase: &CachedPhrase) -> std::io::Result<()> {
        let header = serde_json::to_vec(phrase).map_err(std::io::Error::other)?;
        let mut out = Vec::with_capacity(4 + header.len() + phrase.pcm.len() * 4);
        out.extend_from_slice(
            &u32::try_from(header.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        out.extend_from_slice(&header);
        for s in &phrase.pcm {
            out.extend_from_slice(&s.to_le_bytes());
        }
        let tmp = self.dir.join(format!("{key}.tmp"));
        fs::write(&tmp, out)?;
        fs::rename(tmp, self.path(key))?;
        self.trim()
    }

    /// Łączny rozmiar (bajty).
    pub fn size(&self) -> u64 {
        self.entries().iter().map(|(_, len, _)| len).sum()
    }

    fn entries(&self) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
        let Ok(rd) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        rd.filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "phrase"))
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                Some((e.path(), m.len(), m.modified().ok()?))
            })
            .collect()
    }

    fn trim(&self) -> std::io::Result<()> {
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|(_, l, _)| l).sum();
        entries.sort_by_key(|(_, _, t)| *t);
        for (path, len, _) in entries {
            if total <= self.max_bytes {
                break;
            }
            fs::remove_file(path)?;
            total -= len;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phrase(n: usize) -> CachedPhrase {
        CachedPhrase {
            marks: vec![WordMark {
                word_idx: 0,
                word: "Gotowe.".into(),
                start_ms: 0,
                end_ms: 300,
            }],
            kind: MarksKind::Estimated,
            engine: "pocket".into(),
            pcm: (0..n).map(|i| i as f32 / n as f32).collect(),
        }
    }

    #[test]
    fn roundtrip_and_lru_trim() {
        let dir = std::env::temp_dir().join(format!("alfa-tts-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let cache = PhraseCache::new(dir.clone(), 9_000).unwrap();
        let k1 = PhraseCache::key("Gotowe.", "pocket|pl-f1|1.000|1.000", 1.0);
        assert_eq!(k1.len(), 64);
        assert_ne!(
            k1,
            PhraseCache::key("Gotowe.", "pocket|pl-f2|1.000|1.000", 1.0)
        );
        assert!(cache.get(&k1).is_none());
        cache.put(&k1, &phrase(1_000)).unwrap();
        assert_eq!(cache.get(&k1).unwrap(), phrase(1_000));
        // Starsza data zapisu bez czekania (LRU po mtime).
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000);
        fs::File::options()
            .write(true)
            .open(cache.path(&k1))
            .unwrap()
            .set_modified(old)
            .unwrap();
        let k2 = PhraseCache::key("Przekazuję Delcie.", "x", 1.0);
        cache.put(&k2, &phrase(1_500)).unwrap();
        assert!(cache.size() <= 9_000);
        assert!(cache.get(&k1).is_none(), "najstarsza wyrzucona");
        assert!(cache.get(&k2).is_some());
        fs::write(dir.join("bad.phrase"), b"xx").unwrap();
        assert!(cache.get("bad").is_none());
        fs::remove_dir_all(&dir).unwrap();
    }
}
