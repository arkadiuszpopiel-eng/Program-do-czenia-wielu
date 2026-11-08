//! Manifest modelu `alfa-embed-v1` (`<katalog>/embed.json`): pliki ONNX i `tokenizer.json` z SHA-256
//! (plik o innym hashu nie jest ładowany), wymiar, limit tokenów, pooling, prefiksy (E5: `query: ` /
//! `passage: `), wsad, szacunek RAM dla `model-residency`.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::EmbedError;
use crate::pool::Pooling;

/// Wersja formatu manifestu.
pub const EMBED_MANIFEST_FORMAT: &str = "alfa-embed-v1";

/// Domyślna nazwa pliku manifestu w katalogu modelu.
pub const MANIFEST_FILE: &str = "embed.json";

/// Plik modelu z hashem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    /// Ścieżka względna do katalogu manifestu (bez `..`).
    pub path: String,
    /// SHA-256 (64 znaki hex).
    pub sha256: String,
}

/// Manifest embeddera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedManifest {
    /// [`EMBED_MANIFEST_FORMAT`].
    pub format: String,
    /// Identyfikator modelu (`[A-Za-z0-9._-]`), np. `multilingual-e5-small`.
    pub id: String,
    /// Licencja modelu (SPDX), np. `MIT`.
    pub license: String,
    /// Model ONNX (`input_ids`, opcjonalnie `attention_mask`, `token_type_ids`).
    pub model: FileRef,
    /// `tokenizer.json` (HF, Unigram).
    pub tokenizer: FileRef,
    /// Wymiar wektora (zgodny z `vec0`).
    pub dims: usize,
    /// Limit tokenów tekstu (z tokenami specjalnymi); dłuższy tekst jest obcinany.
    pub max_tokens: usize,
    /// Pooling.
    #[serde(default)]
    pub pooling: Pooling,
    /// Normalizacja L2 (wymagana przez `search`: kosinus = iloczyn skalarny).
    #[serde(default = "yes")]
    pub normalize: bool,
    /// Prefiks zapytań (E5: `query: `).
    #[serde(default)]
    pub query_prefix: String,
    /// Prefiks dokumentów (E5: `passage: `).
    #[serde(default)]
    pub passage_prefix: String,
    /// Nazwa wyjścia modelu (domyślnie pierwsze, zwykle `last_hidden_state`).
    #[serde(default)]
    pub output: Option<String>,
    /// Maksymalna liczba tekstów w jednym przebiegu modelu.
    #[serde(default = "default_batch")]
    pub batch_size: usize,
    /// Szacunek RAM po załadowaniu (MB) — dzierżawa `model-residency`.
    pub ram_mb: u32,
    /// Zwolnienie modelu po bezczynności (s); 0 = nigdy.
    #[serde(default = "default_idle")]
    pub idle_unload_s: u64,
}

fn yes() -> bool {
    true
}

fn default_batch() -> usize {
    8
}

fn default_idle() -> u64 {
    300
}

/// SHA-256 (hex, małe litery).
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 pliku czytanego strumieniowo (hex).
pub fn sha256_file(path: &Path) -> Result<String, EmbedError> {
    let mut file = std::fs::File::open(path).map_err(|e| EmbedError::io(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(|e| EmbedError::io(path, e))?;
        if n == 0 {
            return Ok(hex(&hasher.finalize()));
        }
        hasher.update(&buf[..n]);
    }
}

/// Czy tekst to 64 znaki hex.
pub fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// Czy ścieżka jest względna, bez `..`, bez separatorów Windows i dysków.
pub fn is_safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

impl EmbedManifest {
    /// Parsuje i waliduje manifest.
    pub fn parse(json: &str) -> Result<Self, EmbedError> {
        let m: Self =
            serde_json::from_str(json).map_err(|e| EmbedError::Manifest(e.to_string()))?;
        m.validate()?;
        Ok(m)
    }

    /// Walidacja pól.
    pub fn validate(&self) -> Result<(), EmbedError> {
        let fail = |what: &str| Err(EmbedError::Manifest(what.to_owned()));
        if self.format != EMBED_MANIFEST_FORMAT {
            return fail("format musi być alfa-embed-v1");
        }
        let id_ok = !self.id.is_empty()
            && self.id.len() <= 64
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if !id_ok {
            return fail("id: 1–64 znaki [A-Za-z0-9._-]");
        }
        for f in [&self.model, &self.tokenizer] {
            if !is_safe_relative(&f.path) || !is_sha256(&f.sha256) {
                return fail("pliki: ścieżka względna bez `..` i sha256 (64 hex)");
            }
        }
        if !(8..=4096).contains(&self.dims) || !(8..=8192).contains(&self.max_tokens) {
            return fail("dims 8–4096, max_tokens 8–8192");
        }
        if !(1..=64).contains(&self.batch_size) || !(1..=16_384).contains(&self.ram_mb) {
            return fail("batch_size 1–64, ram_mb 1–16384");
        }
        if self.query_prefix.chars().count() > 64 || self.passage_prefix.chars().count() > 64 {
            return fail("prefiksy ≤ 64 znaki");
        }
        if !self.normalize {
            return fail(
                "normalize = false nie jest obsługiwane (search wymaga wektorów jednostkowych)",
            );
        }
        Ok(())
    }

    /// Wczytuje manifest z pliku; zwraca manifest i katalog bazowy plików.
    pub fn load(path: &Path) -> Result<(Self, PathBuf), EmbedError> {
        let json = std::fs::read_to_string(path).map_err(|e| EmbedError::io(path, e))?;
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        Ok((Self::parse(&json)?, dir))
    }

    /// Identyfikator zapisywany w indeksie: `id@<12 hex>` z odcisku modelu, tokenizera i ustawień
    /// wpływających na wektory (każda zmiana → inny identyfikator → reindeksacja).
    pub fn model_id(&self) -> String {
        let fingerprint = format!(
            "{}|{}|{:?}|{}|{}|{}|{}|{:?}",
            self.model.sha256.to_ascii_lowercase(),
            self.tokenizer.sha256.to_ascii_lowercase(),
            self.pooling,
            self.max_tokens,
            self.query_prefix,
            self.passage_prefix,
            self.dims,
            self.output,
        );
        format!("{}@{}", self.id, &sha256_hex(fingerprint.as_bytes())[..12])
    }

    /// Czyta plik i sprawdza SHA-256 (porównanie bez względu na wielkość liter).
    pub fn read_verified(dir: &Path, file: &FileRef) -> Result<Vec<u8>, EmbedError> {
        let path = dir.join(&file.path);
        let bytes = std::fs::read(&path).map_err(|e| EmbedError::io(&path, e))?;
        let actual = sha256_hex(&bytes);
        if actual.eq_ignore_ascii_case(&file.sha256) {
            Ok(bytes)
        } else {
            Err(EmbedError::Hash {
                path: path.display().to_string(),
                expected: file.sha256.to_ascii_lowercase(),
                actual,
            })
        }
    }

    /// JSON manifestu (ładny, z końcem linii).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> EmbedManifest {
        EmbedManifest {
            format: EMBED_MANIFEST_FORMAT.into(),
            id: "multilingual-e5-small".into(),
            license: "MIT".into(),
            model: FileRef {
                path: "onnx/model.onnx".into(),
                sha256: "a".repeat(64),
            },
            tokenizer: FileRef {
                path: "tokenizer.json".into(),
                sha256: "B".repeat(64),
            },
            dims: 384,
            max_tokens: 512,
            pooling: Pooling::Mean,
            normalize: true,
            query_prefix: "query: ".into(),
            passage_prefix: "passage: ".into(),
            output: None,
            batch_size: 8,
            ram_mb: 600,
            idle_unload_s: 300,
        }
    }

    #[test]
    fn round_trip_and_defaults() {
        let m = sample();
        assert_eq!(EmbedManifest::parse(&m.to_json()).unwrap(), m);
        let minimal = r#"{"format":"alfa-embed-v1","id":"x","license":"MIT",
            "model":{"path":"m.onnx","sha256":"0000000000000000000000000000000000000000000000000000000000000000"},
            "tokenizer":{"path":"t.json","sha256":"0000000000000000000000000000000000000000000000000000000000000000"},
            "dims":16,"max_tokens":32,"ram_mb":10}"#;
        let parsed = EmbedManifest::parse(minimal).unwrap();
        assert_eq!(parsed.pooling, Pooling::Mean);
        assert_eq!((parsed.batch_size, parsed.idle_unload_s), (8, 300));
        assert!(parsed.normalize && parsed.query_prefix.is_empty());
    }

    #[test]
    fn invalid_manifests_are_rejected() {
        let bad = |f: &dyn Fn(&mut EmbedManifest)| {
            let mut m = sample();
            f(&mut m);
            EmbedManifest::parse(&m.to_json()).is_err()
        };
        assert!(bad(&|m| m.format = "v0".into()));
        assert!(bad(&|m| m.id = "a b".into()));
        assert!(bad(&|m| m.model.path = "../x.onnx".into()));
        assert!(bad(&|m| m.model.path = "C:\\x.onnx".into()));
        assert!(bad(&|m| m.tokenizer.path = "/abs.json".into()));
        assert!(bad(&|m| m.tokenizer.sha256 = "zz".into()));
        assert!(bad(&|m| m.dims = 4));
        assert!(bad(&|m| m.max_tokens = 100_000));
        assert!(bad(&|m| m.batch_size = 0));
        assert!(bad(&|m| m.ram_mb = 0));
        assert!(bad(&|m| m.query_prefix = "x".repeat(65)));
        assert!(bad(&|m| m.normalize = false));
        assert!(EmbedManifest::parse("{}").is_err());
        assert!(
            EmbedManifest::parse(
                &sample()
                    .to_json()
                    .replace("\"ram_mb\"", "\"extra\": 1, \"ram_mb\"")
            )
            .is_err()
        );
    }

    #[test]
    fn model_id_tracks_vector_affecting_settings() {
        let base = sample().model_id();
        assert!(base.starts_with("multilingual-e5-small@") && base.len() == 22 + 12);
        let mut m = sample();
        m.query_prefix = "zapytanie: ".into();
        assert_ne!(m.model_id(), base);
        let mut m = sample();
        m.model.sha256 = "A".repeat(64);
        assert_eq!(m.model_id(), base, "wielkość liter hasha bez znaczenia");
        let mut m = sample();
        m.batch_size = 2;
        m.ram_mb = 1;
        assert_eq!(m.model_id(), base, "wsad i RAM nie zmieniają wektorów");
    }

    #[test]
    fn verified_read_checks_hash() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("t.json"), b"abc").unwrap();
        let good = FileRef {
            path: "t.json".into(),
            sha256: sha256_hex(b"abc").to_uppercase(),
        };
        assert_eq!(
            EmbedManifest::read_verified(dir.path(), &good).unwrap(),
            b"abc"
        );
        assert_eq!(
            sha256_file(&dir.path().join("t.json")).unwrap(),
            sha256_hex(b"abc")
        );
        let wrong = FileRef {
            sha256: "0".repeat(64),
            ..good
        };
        assert!(matches!(
            EmbedManifest::read_verified(dir.path(), &wrong),
            Err(EmbedError::Hash { .. })
        ));
        let missing = FileRef {
            path: "nie-ma.json".into(),
            ..wrong
        };
        assert!(matches!(
            EmbedManifest::read_verified(dir.path(), &missing),
            Err(EmbedError::Io { .. })
        ));
        assert!(EmbedManifest::load(&dir.path().join("brak.json")).is_err());
        assert!(sha256_file(&dir.path().join("brak")).is_err());
    }
}
