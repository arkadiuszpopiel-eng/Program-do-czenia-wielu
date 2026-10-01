//! Typy pamięci i trait `Memory`.

use std::fmt;

use chrono::{DateTime, Utc};
use core_bus_contract::{AgentId, SessionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use crate::error::MemoryError;
use crate::model::{Origin, Supersession};

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
    /// Temat faktu (klucz sprzeczności, np. „ulubiony kolor”; F7). Fakt o tym samym temacie
    /// w tym samym zakresie tworzy nową wersję zamiast nadpisania.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Pochodzenie: sesja/tura źródłowa, wpisy źródłowe (F7).
    #[serde(default, skip_serializing_if = "Origin::is_empty")]
    pub origin: Origin,
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
            subject: None,
            origin: Origin::default(),
        }
    }

    /// Wpis w dowolnym zakresie i warstwie (pewność 1, bez TTL, bez encji).
    pub fn new(
        scope: MemoryScope,
        layer: Layer,
        text: impl Into<String>,
        provenance: Provenance,
    ) -> Self {
        Self {
            scope,
            layer,
            text: text.into(),
            entities: Vec::new(),
            provenance,
            confidence: 1.0,
            ttl_secs: None,
            subject: None,
            origin: Origin::default(),
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
    /// Temat faktu (klucz sprzeczności; F7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Pochodzenie: sesja, tura, wpisy źródłowe, rodzaj wyprowadzenia (F7).
    #[serde(default, skip_serializing_if = "Origin::is_empty")]
    pub origin: Origin,
    /// Numer wersji faktu (1 = pierwsza; edycja/sprzeczność → +1).
    #[serde(default = "first_version")]
    pub version: u32,
    /// Poprzednia wersja (ten sam zakres).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<MemoryId>,
    /// Zastąpiony nowszą wersją lub scalony (zostaje w historii, nie wraca w `recall`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded: Option<Supersession>,
    /// Przypięty (warstwa robocza: zawsze w zestawie roboczym).
    #[serde(default)]
    pub pinned: bool,
    /// Kiedy Strażniczka pamięci przetworzyła wpis (konsolidacja epizodów).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consolidated_at: Option<DateTime<Utc>>,
}

fn first_version() -> u32 {
    1
}

impl MemoryEntry {
    /// Wpis z nowego (wspólne dla `-impl`, `-fake` i silnika F7): wersja 1, nieprzypięty
    /// (poza warstwą roboczą), bez zastąpienia.
    pub fn from_new(
        id: MemoryId,
        new: NewMemory,
        created_at: DateTime<Utc>,
        approved: bool,
    ) -> Self {
        Self {
            id,
            trusted: new.provenance.is_trusted(),
            pinned: new.layer == Layer::Working,
            scope: new.scope,
            layer: new.layer,
            text: new.text,
            entities: new.entities,
            provenance: new.provenance,
            confidence: new.confidence,
            ttl_secs: new.ttl_secs,
            created_at,
            approved,
            subject: new.subject,
            origin: new.origin,
            version: 1,
            supersedes: None,
            superseded: None,
            consolidated_at: None,
        }
    }

    /// Odwołanie do wpisu.
    pub fn entry_ref(&self) -> crate::EntryRef {
        crate::EntryRef::new(self.scope.clone(), self.id.clone())
    }
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
