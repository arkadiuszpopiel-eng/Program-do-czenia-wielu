//! Typy pamięci i trait `Memory`.

use std::fmt;

use chrono::{DateTime, Utc};
use core_bus_contract::{AgentId, SessionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Zakres pamięci (PLAN §10). Domyślny: sesja — każdy czat ma osobną pamięć.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "scope", content = "id", rename_all = "snake_case")]
pub enum MemoryScope {
    /// Pamięć sesji (v0).
    Session(SessionId),
    /// Pamięć projektu (F7).
    Project(String),
    /// Pamięć globalna (F7; osobna szyfrowana baza).
    Global,
    /// Pamięć agentki (F7).
    Agent(AgentId),
}

/// Warstwa pamięci.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    /// Robocza (RAM + checkpoint) — poza v0.
    Working,
    /// Epizodyczna (zdarzenia, streszczenia).
    Episodic,
    /// Semantyczna (fakty, preferencje).
    Semantic,
    /// Proceduralna (umiejętności, pliki) — poza v0.
    Procedural,
}

/// Pochodzenie wpisu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    /// Użytkownik (zaufane).
    User,
    /// Agentka (zaufane — z rozmowy, nie z treści zewnętrznej).
    Agent {
        /// Agentka.
        agent: AgentId,
    },
    /// Treść niezaufana (strona WWW, plik, e-mail…) — oznaczona, bez automatycznego zapamiętania
    /// i bez awansu zakresu.
    UntrustedContent {
        /// Źródło (URL, ścieżka…).
        source: String,
    },
    /// Import (paczka `.alfa`).
    Import {
        /// Źródło importu.
        source: String,
    },
}

impl Provenance {
    /// Czy wpis pochodzi z treści zaufanej.
    pub fn is_trusted(&self) -> bool {
        !matches!(self, Provenance::UntrustedContent { .. })
    }
}

/// Identyfikator wpisu.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct MemoryId(pub String);

impl fmt::Display for MemoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Nowy wpis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NewMemory {
    /// Zakres.
    pub scope: MemoryScope,
    /// Warstwa.
    pub layer: Layer,
    /// Treść.
    pub text: String,
    /// Encje (nazwy).
    pub entities: Vec<String>,
    /// Pochodzenie.
    pub provenance: Provenance,
    /// Pewność 0–1.
    pub confidence: f32,
    /// Czas życia w sekundach (`None` = bez wygasania; SPEC: domyślnie 90 dni ustawia wywołujący).
    pub ttl_secs: Option<u64>,
}

impl NewMemory {
    /// Fakt semantyczny od użytkownika w sesji (pewność 1, bez TTL).
    pub fn user_fact(session: SessionId, text: impl Into<String>) -> Self {
        Self {
            scope: MemoryScope::Session(session),
            layer: Layer::Semantic,
            text: text.into(),
            entities: Vec::new(),
            provenance: Provenance::User,
            confidence: 1.0,
            ttl_secs: None,
        }
    }
}

/// Wpis pamięci.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MemoryEntry {
    /// Identyfikator.
    pub id: MemoryId,
    /// Zakres.
    pub scope: MemoryScope,
    /// Warstwa.
    pub layer: Layer,
    /// Treść.
    pub text: String,
    /// Encje.
    pub entities: Vec<String>,
    /// Pochodzenie.
    pub provenance: Provenance,
    /// Czy z treści zaufanej (kopia `provenance.is_trusted()` — flaga widoczna w Inspektorze).
    pub trusted: bool,
    /// Pewność 0–1.
    pub confidence: f32,
    /// Czas życia w sekundach.
    pub ttl_secs: Option<u64>,
    /// Utworzenie.
    pub created_at: DateTime<Utc>,
    /// Zatwierdzony (tryb `AutoPendingApproval` → `false` do czasu `approve`).
    pub approved: bool,
}

/// Tryb zapamiętania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RememberMode {
    /// Jawna akcja użytkownika („zapamiętaj”) — od razu zatwierdzony.
    Explicit,
    /// Ekstrakcja automatyczna — czeka na zatwierdzenie; zabroniona dla treści niezaufanej.
    AutoPendingApproval,
}

/// Wynik `recall`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Recalled {
    /// Wpis.
    pub entry: MemoryEntry,
    /// Wynik trafności (większy = lepszy).
    pub score: f32,
}

/// Raport kaskady `forget` (weryfikowalny — PLAN §10).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForgetReport {
    /// Usunięty wpis.
    pub entry: bool,
    /// Usunięte wiersze FTS.
    pub fts_rows: usize,
    /// Usunięte wektory.
    pub vectors: usize,
    /// Usunięte streszczenia/kopie (F7; w v0 zawsze 0).
    pub derived: usize,
}

/// Błędy pamięci.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum MemoryError {
    /// Funkcja poza v0 (zakres inny niż sesja, warstwa robocza/proceduralna, awans).
    #[error("nieobsługiwane w v0: {what}")]
    Unsupported {
        /// Czego dotyczy.
        what: String,
    },
    /// Wpis nie istnieje.
    #[error("wpis {id} nie istnieje")]
    NotFound {
        /// Identyfikator.
        id: MemoryId,
    },
    /// Wpis z treści niezaufanej nie może awansować do innego zakresu.
    #[error("wpis z treści niezaufanej nie może awansować")]
    UntrustedCannotPromote,
    /// Automatyczne zapamiętywanie z treści niezaufanej jest wyłączone.
    #[error("automatyczne zapamiętywanie z treści niezaufanej jest wyłączone")]
    UntrustedAutoRemember,
    /// Nieprawidłowe dane.
    #[error("nieprawidłowe dane: {reason}")]
    Invalid {
        /// Opis.
        reason: String,
    },
    /// Błąd magazynu lub indeksu.
    #[error("magazyn: {reason}")]
    Storage {
        /// Opis.
        reason: String,
    },
}

impl MemoryError {
    /// Skrót: błąd magazynu.
    pub fn storage(e: impl fmt::Display) -> Self {
        MemoryError::Storage {
            reason: e.to_string(),
        }
    }
}

/// Pamięć agentek.
pub trait Memory: Send + Sync {
    /// Zapamiętuje wpis (walidacja: [`crate::validate_new`]).
    fn remember(&self, new: NewMemory, mode: RememberMode) -> Result<MemoryEntry, MemoryError>;
    /// Hybryda FTS + wektor w podanych zakresach; tylko wpisy zatwierdzone i niewygasłe.
    fn recall(
        &self,
        scopes: &[MemoryScope],
        query: &str,
        k: usize,
    ) -> Result<Vec<Recalled>, MemoryError>;
    /// Jeden wpis.
    fn get(&self, scope: &MemoryScope, id: &MemoryId) -> Result<MemoryEntry, MemoryError>;
    /// Wszystkie wpisy zakresu (Inspektor pamięci), rosnąco po utworzeniu.
    fn list(&self, scope: &MemoryScope) -> Result<Vec<MemoryEntry>, MemoryError>;
    /// Zatwierdza wpis oczekujący.
    fn approve(&self, scope: &MemoryScope, id: &MemoryId) -> Result<MemoryEntry, MemoryError>;
    /// Zapomina kaskadowo (wpis + FTS + wektor).
    fn forget(&self, scope: &MemoryScope, id: &MemoryId) -> Result<ForgetReport, MemoryError>;
    /// Awans zakresu (za zgodą użytkownika). v0: niezaufane → odmowa, pozostałe → `Unsupported`.
    fn promote(
        &self,
        scope: &MemoryScope,
        id: &MemoryId,
        to: MemoryScope,
    ) -> Result<MemoryEntry, MemoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_display_and_tagged_serde() {
        assert!(Provenance::User.is_trusted());
        assert!(
            Provenance::Agent {
                agent: AgentId::new("alfa")
            }
            .is_trusted()
        );
        assert!(!Provenance::UntrustedContent { source: "x".into() }.is_trusted());
        assert_eq!(MemoryId("m1".into()).to_string(), "m1");
        assert_eq!(MemoryError::storage("dysk").to_string(), "magazyn: dysk");
        let scope = serde_json::to_value(MemoryScope::Session(SessionId::new("s"))).unwrap();
        assert_eq!(scope, serde_json::json!({"scope": "session", "id": "s"}));
        let err = serde_json::to_value(MemoryError::UntrustedCannotPromote).unwrap();
        assert_eq!(
            err,
            serde_json::json!({"error": "untrusted_cannot_promote"})
        );
    }
}
