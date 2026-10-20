//! DTO „Naucz Alfę” (P5; odpowiedniki `types-memory.ts`): wiedza z wklejonego tekstu, pliku
//! (txt/md/pdf/docx) albo bieżącej rozmowy trafia do pamięci jako fakty ze źródłem. Tekst i plik
//! to treść z zewnątrz — niezaufana (dane, nie polecenia), więc tylko zakres sesji.

use serde::{Deserialize, Serialize};

use super::sessions::RememberScope;

/// Źródło wiedzy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TeachSource {
    Text { text: String },
    File { name: String, text: String },
    Conversation,
}

/// Żądanie „Naucz Alfę” (`title` puste — nazwa pliku, tytuł rozmowy albo „Wklejony tekst”).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TeachRequest {
    pub session_id: String,
    pub source: TeachSource,
    pub scope: RememberScope,
    pub title: Option<String>,
}

/// Wynik: ile faktów zapisano (identyfikatory do cofnięcia przez `memory_forget`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TeachReport {
    pub facts: u64,
    pub ids: Vec<String>,
    pub scope_key: String,
    /// Źródło zapisane przy faktach (np. `naucz:plik:raport.pdf#3f2a…`, `rozmowa:<sesja>`).
    pub source: String,
    pub trusted: bool,
    /// Treść ucięta limitem znaków albo liczby faktów.
    pub truncated: bool,
    /// Powtórzone fragmenty pominięte.
    pub duplicates: u64,
}

/// Plik wybrany do nauki: wyciągnięty tekst (podgląd przed zapisem).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TeachFile {
    pub name: String,
    pub text: String,
    pub chars: u64,
    pub truncated: bool,
}
