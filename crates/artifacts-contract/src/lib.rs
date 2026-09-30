//! Kontrakt modułu `artifacts` (docs/modules/artifacts/SPEC.md, PLAN §11).
//!
//! Rejestr plików wyjściowych sesji: ścieżka, rozmiar, MIME, SHA-256, **niezmienne wersje**
//! (nowa wersja = nowy hash), tura źródłowa. Podgląd tekstowy (pierwsze N bajtów, wykrycie
//! binarnych), diff tekstowy między wersjami, a akcje UI (Otwórz, Pokaż w Eksploratorze, Kopiuj
//! jako plik, Zapisz jako, Spakuj, Przekaż do sesji) jako **intencje** — wykonuje je
//! `platform-windows` jako użytkownik, nie agentka.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod types;
mod util;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use types::{
    Artifact, ArtifactAction, ArtifactError, ArtifactId, ArtifactIntent, ArtifactVersion,
    Artifacts, DiffLine, DiffTag, Origin, Preview, TextDiff,
};
pub use util::{
    BINARY_SNIFF_BYTES, DEFAULT_PREVIEW_BYTES, DEFAULT_SNAPSHOT_MAX_BYTES, FileFacts,
    default_out_dir, diff_contents, diff_texts, guess_mime, looks_binary, next_version,
    preview_bytes, read_file_facts, sha256_hex, validate_action, version_content,
};

pub use sessions_contract::{SessionId, TurnId};

/// Nazwy zdarzeń modułu (ładunki bez treści plików).
pub mod events {
    /// Zarejestrowano artefakt (`{ "artifact", "version", "mime", "bytes" }`).
    pub const REGISTERED: &str = "artifact.registered";
    /// Dodano wersję.
    pub const VERSION_ADDED: &str = "artifact.version.added";
    /// Utworzono intencję eksportu (Zapisz jako, Spakuj, Kopiuj jako plik…).
    pub const EXPORTED: &str = "artifact.exported";
    /// Utworzono intencję przekazania do innej sesji.
    pub const HANDOFF: &str = "artifact.handoff";
}
