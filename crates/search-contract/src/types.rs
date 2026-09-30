//! Typy dokumentów, zapytań i trafień.

use std::fmt;

use chrono::{DateTime, Utc};
use core_bus_contract::SessionId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Rodzaj dokumentu w indeksie.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DocKind {
    /// Tura rozmowy.
    Turn,
    /// Wpis pamięci.
    Memory,
    /// Artefakt (plik wyjściowy).
    Artifact,
}

impl DocKind {
    /// Wszystkie rodzaje.
    pub const ALL: [DocKind; 3] = [DocKind::Turn, DocKind::Memory, DocKind::Artifact];

    /// Nazwa tekstowa (kolumna `kind` w bazie).
    pub fn as_str(self) -> &'static str {
        match self {
            DocKind::Turn => "turn",
            DocKind::Memory => "memory",
            DocKind::Artifact => "artifact",
        }
    }

    /// Parsuje nazwę tekstową.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// Identyfikator dokumentu: rodzaj + klucz w obrębie sesji (np. numer tury, id wpisu pamięci).
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct DocId {
    /// Rodzaj.
    pub kind: DocKind,
    /// Klucz w obrębie rodzaju i sesji.
    pub key: String,
}

impl DocId {
    /// Nowy identyfikator.
    pub fn new(kind: DocKind, key: impl Into<String>) -> Self {
        Self {
            kind,
            key: key.into(),
        }
    }
}

impl fmt::Display for DocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind.as_str(), self.key)
    }
}

/// Dokument do zaindeksowania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Doc {
    /// Identyfikator (ponowne `index` z tym samym id zastępuje dokument).
    pub id: DocId,
    /// Sesja (baza), do której należy dokument.
    pub session: SessionId,
    /// Tekst (oryginalny; indeks przechowuje też formę złożoną `fold_pl`).
    pub text: String,
    /// Znacznik czasu dokumentu.
    pub ts: DateTime<Utc>,
}

/// Tryb zapytania.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Pełnotekstowe (FTS5, bez diakrytyków, prefiksy słów, AND).
    Fts,
    /// Wektorowe (kNN po embeddingu, kosinus).
    Vector,
    /// Hybryda: FTS + wektor złączone przez RRF.
    #[default]
    Hybrid,
}

/// Zbiór przeszukiwanych sesji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "scope", content = "sessions", rename_all = "snake_case")]
pub enum SessionSet {
    /// Jedna sesja.
    One(SessionId),
    /// Wybrane sesje (tylko właściciel).
    Many(Vec<SessionId>),
    /// Wszystkie sesje (tylko właściciel — „szukaj wszędzie”, otwiera wiele baz).
    All,
}

/// Kto pyta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "caller", rename_all = "snake_case")]
pub enum Caller {
    /// Właściciel przez UI.
    Owner,
    /// Agentka pracująca w sesji `session` (widzi tylko tę sesję).
    Agent {
        /// Sesja agentki.
        session: SessionId,
    },
}

/// Zapytanie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Query {
    /// Tekst zapytania (składnia FTS5 użytkownika nie przechodzi — słowa są cytowane).
    pub text: String,
    /// Sesje.
    pub sessions: SessionSet,
    /// Tryb.
    pub mode: Mode,
    /// Maksymalna liczba trafień (przycinana do [`crate::MAX_LIMIT`]; 0 → brak trafień).
    pub limit: usize,
    /// Rodzaje dokumentów (pusta lista = wszystkie).
    pub kinds: Vec<DocKind>,
}

impl Query {
    /// Zapytanie w jednej sesji, tryb hybrydowy, wszystkie rodzaje.
    pub fn in_session(session: SessionId, text: impl Into<String>, limit: usize) -> Self {
        Self {
            text: text.into(),
            sessions: SessionSet::One(session),
            mode: Mode::Hybrid,
            limit,
            kinds: Vec::new(),
        }
    }

    /// Czy rodzaj przechodzi przez filtr.
    pub fn accepts(&self, kind: DocKind) -> bool {
        self.kinds.is_empty() || self.kinds.contains(&kind)
    }
}

/// Podświetlony zakres w [`Snippet::text`] (indeksy znaków, nie bajtów; `end` wyłącznie).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Highlight {
    /// Początek.
    pub start: usize,
    /// Koniec (wyłącznie).
    pub end: usize,
}

/// Fragment dokumentu z podświetleniami (dane strukturalne — UI nie wstawia HTML).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Snippet {
    /// Tekst fragmentu (z „…” na obciętych końcach, białe znaki sterujące → spacje).
    pub text: String,
    /// Zakresy trafień w `text`.
    pub highlights: Vec<Highlight>,
}

/// Trafienie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hit {
    /// Dokument.
    pub doc: DocId,
    /// Sesja.
    pub session: SessionId,
    /// Wynik (większy = lepszy; skala zależna od trybu).
    pub score: f32,
    /// Fragment z podświetleniem.
    pub snippet: Snippet,
}

/// Wynik usunięcia dokumentu z indeksu (dowód kaskady).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RemoveReport {
    /// Usunięte dokumenty (0 albo 1).
    pub docs: usize,
    /// Usunięte wiersze FTS.
    pub fts_rows: usize,
    /// Usunięte wektory.
    pub vectors: usize,
}
