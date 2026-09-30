//! Typy artefaktów, podglądu, diffu i intencji oraz trait `Artifacts`.

use std::fmt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use core_bus_contract::AgentId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sessions_contract::{SessionId, TurnId};

/// Identyfikator artefaktu.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ArtifactId(pub String);

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Kto wytworzył artefakt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    /// Agentka.
    Agent {
        /// Agentka.
        agent: AgentId,
    },
    /// Użytkownik (wejście: przeciągnięcie, wklejenie, zrzut).
    User,
    /// Import (np. „Otwórz w Alfie”, paczka `.alfa`).
    Import {
        /// Źródło.
        source: String,
    },
}

/// Niezmienna wersja artefaktu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactVersion {
    /// Numer wersji (od 1).
    pub version: u32,
    /// Ścieżka pliku w chwili rejestracji.
    pub path: PathBuf,
    /// Rozmiar w bajtach.
    pub bytes: u64,
    /// SHA-256 (hex, małe litery).
    pub sha256: String,
    /// Typ MIME (z rozszerzenia).
    pub mime: String,
    /// Tura, która wytworzyła wersję.
    pub source_turn: Option<TurnId>,
    /// Rejestracja wersji.
    pub created_at: DateTime<Utc>,
    /// Czy treść wersji jest zachowana w bazie sesji (≤ limit migawki) — wtedy podgląd i diff
    /// działają także po nadpisaniu pliku.
    pub snapshot: bool,
}

/// Artefakt sesji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Artifact {
    /// Identyfikator.
    pub id: ArtifactId,
    /// Sesja.
    pub session: SessionId,
    /// Nazwa (nazwa pliku).
    pub name: String,
    /// Pochodzenie.
    pub origin: Origin,
    /// Wersje rosnąco (co najmniej jedna).
    pub versions: Vec<ArtifactVersion>,
}

impl Artifact {
    /// Najnowsza wersja.
    pub fn latest(&self) -> Option<&ArtifactVersion> {
        self.versions.last()
    }

    /// Wersja o numerze.
    pub fn version(&self, version: u32) -> Option<&ArtifactVersion> {
        self.versions.iter().find(|v| v.version == version)
    }
}

/// Podgląd wersji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Preview {
    /// Tekst (pierwsze N bajtów, ucięte na granicy znaku).
    Text {
        /// Treść.
        text: String,
        /// Czy ucięto.
        truncated: bool,
    },
    /// Plik binarny (podgląd przez protokół zasobów w UI, nie przez IPC).
    Binary {
        /// Typ MIME.
        mime: String,
        /// Rozmiar.
        bytes: u64,
    },
}

/// Rodzaj linii diffu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiffTag {
    /// Bez zmian.
    Equal,
    /// Dodana.
    Insert,
    /// Usunięta.
    Delete,
}

/// Linia diffu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiffLine {
    /// Rodzaj.
    pub tag: DiffTag,
    /// Numer linii w starej wersji (od 1).
    pub old_line: Option<usize>,
    /// Numer linii w nowej wersji (od 1).
    pub new_line: Option<usize>,
    /// Treść linii (bez końca linii).
    pub text: String,
}

/// Diff tekstowy dwóch wersji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TextDiff {
    /// Linie.
    pub lines: Vec<DiffLine>,
    /// Liczba linii dodanych.
    pub added: usize,
    /// Liczba linii usuniętych.
    pub removed: usize,
    /// Format ujednolicony (`diff -u`, 3 linie kontekstu).
    pub unified: String,
}

/// Akcja UI na artefakcie (wykonuje ją `platform-windows` jako użytkownik).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ArtifactAction {
    /// Otwórz domyślną aplikacją.
    Open,
    /// Pokaż w Eksploratorze.
    Reveal,
    /// Kopiuj jako plik do schowka (CF_HDROP).
    CopyAsFile,
    /// Zapisz jako…
    SaveAs {
        /// Ścieżka docelowa (wybrana w oknie dialogowym).
        target: PathBuf,
    },
    /// Spakuj do ZIP.
    Zip {
        /// Ścieżka archiwum.
        target: PathBuf,
    },
    /// Przekaż do innej sesji (kopia + jawna tura `Handoff` w sesji docelowej).
    SendToSession {
        /// Sesja docelowa.
        target: SessionId,
    },
}

/// Zwalidowana intencja dla `platform-windows`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactIntent {
    /// Sesja.
    pub session: SessionId,
    /// Artefakt.
    pub artifact: ArtifactId,
    /// Wersja.
    pub version: u32,
    /// Plik źródłowy.
    pub path: PathBuf,
    /// SHA-256 wersji (wykonawca sprawdza, czy plik nie zmienił się od rejestracji).
    pub sha256: String,
    /// Akcja.
    pub action: ArtifactAction,
}

/// Błędy artefaktów.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum ArtifactError {
    /// Artefakt nie istnieje.
    #[error("artefakt {id} nie istnieje")]
    NotFound {
        /// Identyfikator.
        id: ArtifactId,
    },
    /// Wersja nie istnieje.
    #[error("artefakt {id} nie ma wersji {version}")]
    VersionNotFound {
        /// Identyfikator.
        id: ArtifactId,
        /// Wersja.
        version: u32,
    },
    /// Plik nie istnieje albo nie jest zwykłym plikiem.
    #[error("plik {path} nie istnieje albo nie jest zwykłym plikiem")]
    FileNotFound {
        /// Ścieżka.
        path: PathBuf,
    },
    /// Wersja nie jest tekstem (diff niemożliwy).
    #[error("wersja nie jest tekstem")]
    NotText,
    /// Treść wersji niedostępna (brak migawki, plik zmieniony).
    #[error("treść wersji niedostępna: {reason}")]
    ContentUnavailable {
        /// Opis.
        reason: String,
    },
    /// Nieprawidłowe dane.
    #[error("nieprawidłowe dane: {reason}")]
    Invalid {
        /// Opis.
        reason: String,
    },
    /// Błąd magazynu / we-wy.
    #[error("magazyn: {reason}")]
    Storage {
        /// Opis.
        reason: String,
    },
}

impl ArtifactError {
    /// Skrót: błąd magazynu.
    pub fn storage(e: impl fmt::Display) -> Self {
        ArtifactError::Storage {
            reason: e.to_string(),
        }
    }
}

/// Rejestr artefaktów sesji.
pub trait Artifacts: Send + Sync {
    /// Katalog wyjściowy sesji: `<root>\Sesje\<dir_name>\out` (`dir_name` = nazwa katalogu
    /// roboczego sesji, `SessionMeta::workdir_name`).
    fn out_dir(&self, dir_name: &str) -> PathBuf;
    /// Rejestruje plik. Ta sama ścieżka w sesji → nowa wersja istniejącego artefaktu
    /// (identyczny hash → bez nowej wersji).
    fn register(
        &self,
        session: &SessionId,
        path: &Path,
        origin: Origin,
        source_turn: Option<TurnId>,
    ) -> Result<Artifact, ArtifactError>;
    /// Nowa wersja z pliku `path` (identyczny hash jak najnowsza → bez zmian).
    fn add_version(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        path: &Path,
        source_turn: Option<TurnId>,
    ) -> Result<Artifact, ArtifactError>;
    /// Artefakt.
    fn get(&self, session: &SessionId, id: &ArtifactId) -> Result<Artifact, ArtifactError>;
    /// Artefakty sesji w kolejności rejestracji.
    fn list(&self, session: &SessionId) -> Result<Vec<Artifact>, ArtifactError>;
    /// Podgląd wersji (`None` = najnowsza), co najwyżej `max_bytes` bajtów tekstu.
    fn preview(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        version: Option<u32>,
        max_bytes: usize,
    ) -> Result<Preview, ArtifactError>;
    /// Diff tekstowy wersji `from` → `to`.
    fn diff(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        from: u32,
        to: u32,
    ) -> Result<TextDiff, ArtifactError>;
    /// Intencja akcji UI (wersja `None` = najnowsza).
    fn intent(
        &self,
        session: &SessionId,
        id: &ArtifactId,
        version: Option<u32>,
        action: ArtifactAction,
    ) -> Result<ArtifactIntent, ArtifactError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_lookup_display_and_tagged_serde() {
        let v = |n: u32| ArtifactVersion {
            version: n,
            path: PathBuf::from("a.txt"),
            bytes: 1,
            sha256: "00".into(),
            mime: "text/plain".into(),
            source_turn: None,
            created_at: DateTime::<Utc>::default(),
            snapshot: true,
        };
        let art = Artifact {
            id: ArtifactId("art-1".into()),
            session: SessionId::new("s"),
            name: "a.txt".into(),
            origin: Origin::User,
            versions: vec![v(1), v(2)],
        };
        assert_eq!(art.latest().map(|x| x.version), Some(2));
        assert_eq!(art.version(1).map(|x| x.version), Some(1));
        assert!(art.version(3).is_none());
        assert_eq!(art.id.to_string(), "art-1");
        assert_eq!(ArtifactError::storage("dysk").to_string(), "magazyn: dysk");
        let json = serde_json::to_value(ArtifactAction::SendToSession {
            target: SessionId::new("t"),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"action": "send_to_session", "target": "t"})
        );
        let err = serde_json::to_value(ArtifactError::NotText).unwrap();
        assert_eq!(err, serde_json::json!({"error": "not_text"}));
    }
}
