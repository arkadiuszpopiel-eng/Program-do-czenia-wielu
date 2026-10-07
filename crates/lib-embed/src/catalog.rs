//! Katalog znanych modeli embeddingu (wpisy dla menedżera modeli w `app-*`): skąd pobrać pliki,
//! rozmiar, licencja i ustawienia manifestu. Domyślny: `multilingual-e5-small` (MIT, 384 wym.).
//!
//! **SHA-256:** HuggingFace był niedostępny przy tworzeniu katalogu, więc hashe nie są przypięte
//! (`sha256: None`). Instalator domyślnie ([`crate::install::HashPolicy::PinnedOnly`]) odmawia
//! pobrania takiego pliku; człowiek przypina hash (strona pliku na HF → „SHA256” w metadanych LFS)
//! przed wydaniem. `TrustOnFirstUse` (jawna zgoda użytkownika w UI) zapisuje hash pierwszego
//! pobrania w manifeście — każde późniejsze ładowanie go weryfikuje. Adresy i rozmiary — do
//! potwierdzenia przy przypinaniu (ten sam model ONNX jest też w `Xenova/multilingual-e5-small`).

use crate::manifest::{EMBED_MANIFEST_FORMAT, EmbedManifest, FileRef};
use crate::pool::Pooling;

/// Plik modelu w katalogu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogFile {
    /// Ścieżka w katalogu modelu (i w manifeście).
    pub path: &'static str,
    /// Adres pobrania (https).
    pub url: &'static str,
    /// Przypięty SHA-256 (hex); `None` = do przypięcia przed wydaniem.
    pub sha256: Option<&'static str>,
    /// Przybliżony rozmiar (MB) — komunikat w UI i górny limit pobierania (×2).
    pub size_mb: u32,
}

/// Wpis katalogu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatalogEntry {
    /// Identyfikator (katalog i `id` manifestu).
    pub id: &'static str,
    /// Nazwa wyświetlana.
    pub name: &'static str,
    /// Licencja (SPDX).
    pub license: &'static str,
    /// Strona modelu.
    pub homepage: &'static str,
    /// Model ONNX.
    pub model: CatalogFile,
    /// `tokenizer.json`.
    pub tokenizer: CatalogFile,
    /// Wymiar.
    pub dims: usize,
    /// Limit tokenów.
    pub max_tokens: usize,
    /// Pooling.
    pub pooling: Pooling,
    /// Prefiks zapytań.
    pub query_prefix: &'static str,
    /// Prefiks dokumentów.
    pub passage_prefix: &'static str,
    /// Szacunek RAM po załadowaniu (MB).
    pub ram_mb: u32,
    /// Opis dla UI (języki, jakość, koszt).
    pub note: &'static str,
}

/// Identyfikator modelu domyślnego.
pub const DEFAULT_MODEL: &str = "multilingual-e5-small";

/// Znane modele.
pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: "multilingual-e5-small",
        name: "Multilingual E5 small (ONNX fp32)",
        license: "MIT",
        homepage: "https://huggingface.co/intfloat/multilingual-e5-small",
        model: CatalogFile {
            path: "onnx/model.onnx",
            url: "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/onnx/model.onnx",
            sha256: None,
            size_mb: 471,
        },
        tokenizer: CatalogFile {
            path: "tokenizer.json",
            url: "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/tokenizer.json",
            sha256: None,
            size_mb: 17,
        },
        dims: 384,
        max_tokens: 512,
        pooling: Pooling::Mean,
        query_prefix: "query: ",
        passage_prefix: "passage: ",
        ram_mb: 640,
        note: "~100 języków (w tym polski), zapytanie–dokument (prefiksy query:/passage:), 118 M parametrów; \
               domyślny dla pamięci F7",
    },
    CatalogEntry {
        id: "paraphrase-multilingual-minilm-l12-v2",
        name: "Paraphrase multilingual MiniLM L12 v2 (ONNX fp32)",
        license: "Apache-2.0",
        homepage: "https://huggingface.co/sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2",
        model: CatalogFile {
            path: "onnx/model.onnx",
            url: "https://huggingface.co/sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2/resolve/main/onnx/model.onnx",
            sha256: None,
            size_mb: 471,
        },
        tokenizer: CatalogFile {
            path: "tokenizer.json",
            url: "https://huggingface.co/sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2/resolve/main/tokenizer.json",
            sha256: None,
            size_mb: 17,
        },
        dims: 384,
        max_tokens: 128,
        pooling: Pooling::Mean,
        query_prefix: "",
        passage_prefix: "",
        ram_mb: 640,
        note: "50+ języków, podobieństwo zdań (symetryczne, bez prefiksów), teksty ≤ 128 tokenów; zapasowy",
    },
];

/// Wpis po identyfikatorze.
pub fn find(id: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|e| e.id == id)
}

impl CatalogEntry {
    /// Manifest dla pobranych plików (`hashes` = SHA-256 modelu i tokenizera).
    pub fn manifest(&self, model_sha256: &str, tokenizer_sha256: &str) -> EmbedManifest {
        EmbedManifest {
            format: EMBED_MANIFEST_FORMAT.into(),
            id: self.id.into(),
            license: self.license.into(),
            model: FileRef {
                path: self.model.path.into(),
                sha256: model_sha256.to_ascii_lowercase(),
            },
            tokenizer: FileRef {
                path: self.tokenizer.path.into(),
                sha256: tokenizer_sha256.to_ascii_lowercase(),
            },
            dims: self.dims,
            max_tokens: self.max_tokens,
            pooling: self.pooling,
            normalize: true,
            query_prefix: self.query_prefix.into(),
            passage_prefix: self.passage_prefix.into(),
            output: None,
            batch_size: 8,
            ram_mb: self.ram_mb,
            idle_unload_s: 300,
        }
    }

    /// Pliki wpisu (model, tokenizer).
    pub fn files(&self) -> [CatalogFile; 2] {
        [self.model, self.tokenizer]
    }

    /// Łączny przybliżony rozmiar pobrania (MB).
    pub fn download_mb(&self) -> u32 {
        self.model.size_mb + self.tokenizer.size_mb
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{is_safe_relative, is_sha256};

    #[test]
    fn catalog_entries_are_valid() {
        assert!(find(DEFAULT_MODEL).is_some());
        assert!(find("nie-ma").is_none());
        for e in CATALOG {
            for f in e.files() {
                assert!(f.url.starts_with("https://huggingface.co/"), "{}", f.url);
                assert!(f.url.ends_with(f.path), "{}", f.url);
                assert!(is_safe_relative(f.path));
                assert!(f.sha256.is_none_or(is_sha256));
            }
            let m = e.manifest(&"A".repeat(64), &"b".repeat(64));
            m.validate().unwrap();
            assert_eq!(m.model.sha256, "a".repeat(64));
            assert!(e.download_mb() > 400);
        }
    }
}
