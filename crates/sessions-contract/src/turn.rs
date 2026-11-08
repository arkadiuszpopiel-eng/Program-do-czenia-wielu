//! Tura rozmowy (IR append-only, ADR 0006): rola, autor, treść z blokami, usłyszany prefiks.

use chrono::{DateTime, Utc};
use core_bus_contract::Cost;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::SessionError;
use crate::ids::{AgentId, BranchId, SessionId, TurnId};

/// Rola tury (jak w API dostawców).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Użytkownik.
    User,
    /// Agentka/model.
    Assistant,
    /// Komunikat systemowy.
    System,
    /// Wynik narzędzia.
    Tool,
}

/// Autor tury.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Author {
    /// Użytkownik (Ty).
    User,
    /// Agentka.
    Agent {
        /// Identyfikator agentki.
        agent: AgentId,
    },
    /// System Alfy.
    System,
    /// Narzędzie.
    Tool {
        /// Nazwa narzędzia.
        name: String,
    },
    /// Jawne przekazanie kontekstu z innej sesji (nie współdzielenie pamięci).
    Handoff {
        /// Sesja źródłowa.
        from: SessionId,
    },
}

/// Referencja do załącznika (plik żyje w `artifacts`; tu tylko odnośnik).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttachmentRef {
    /// Nazwa pliku.
    pub name: String,
    /// Typ MIME.
    pub mime: String,
    /// Identyfikator artefaktu, jeśli zarejestrowany.
    pub artifact_id: Option<String>,
    /// SHA-256 (hex) treści.
    pub sha256: Option<String>,
    /// Rozmiar w bajtach.
    pub bytes: Option<u64>,
}

/// Blok treści tury (IR niezależny od dostawcy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// Tekst.
    Text {
        /// Treść.
        text: String,
    },
    /// Myślenie modelu z podpisem dostawcy (nieprzezroczysty; ważny tylko dla nieedytowanej historii).
    Thinking {
        /// Dostawca, który wystawił podpis.
        provider: String,
        /// Treść myślenia.
        text: String,
        /// Podpis dostawcy (nieprzezroczysty, przechowywany bajtowo bez zmian).
        signature: Option<String>,
    },
    /// Myślenie zredagowane przez dostawcę (dane nieprzezroczyste).
    RedactedThinking {
        /// Dostawca.
        provider: String,
        /// Dane nieprzezroczyste.
        data: String,
    },
    /// Wywołanie narzędzia.
    ToolUse {
        /// Identyfikator wywołania.
        id: String,
        /// Nazwa narzędzia.
        name: String,
        /// Argumenty (JSON).
        input: serde_json::Value,
    },
    /// Wynik narzędzia.
    ToolResult {
        /// Identyfikator wywołania, którego dotyczy.
        tool_use_id: String,
        /// Treść wyniku.
        content: String,
        /// Czy wynik jest błędem.
        is_error: bool,
    },
    /// Załącznik.
    Attachment {
        /// Odnośnik.
        attachment: AttachmentRef,
    },
}

/// Treść tury: tekst główny + bloki IR. Dla tury asystentki `text` = **`assistant_full`**
/// (pełna odpowiedź), a usłyszany prefiks jest osobnym faktem ([`HeardPrefix`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TurnContent {
    /// Tekst główny (wyświetlany, indeksowany).
    pub text: String,
    /// Bloki IR (myślenie, narzędzia, załączniki…).
    pub blocks: Vec<Block>,
}

impl TurnContent {
    /// Treść z samym tekstem.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            blocks: Vec::new(),
        }
    }

    /// Czy treść jest pusta (brak tekstu i bloków).
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.blocks.is_empty()
    }

    /// Tekst do indeksu wyszukiwania: tekst główny + bloki `Text` (bez powtórzeń); myślenie,
    /// narzędzia i załączniki nie są indeksowane.
    pub fn searchable_text(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if !self.text.trim().is_empty() {
            parts.push(&self.text);
        }
        for block in &self.blocks {
            if let Block::Text { text } = block
                && !text.trim().is_empty()
                && !parts.contains(&text.as_str())
            {
                parts.push(text);
            }
        }
        parts.join("\n")
    }
}

/// Usłyszany prefiks odpowiedzi (barge-in, PLAN §6.5): długość w znakach tekstu `assistant_full`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HeardPrefix {
    /// Liczba znaków (nie bajtów) usłyszanych przez użytkownika.
    pub chars: usize,
    /// Wyznaczony w przybliżeniu (zliczanie próbek zamiast znaczników słów/alignmentu).
    pub approximate: bool,
}

/// Model, dostawca i koszt wywołania, które wytworzyło turę.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelUsage {
    /// Dostawca (np. `anthropic`).
    pub provider: String,
    /// Model.
    pub model: String,
    /// Tokeny, koszt, opóźnienie.
    pub cost: Cost,
}

/// Nowa tura do dopisania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NewTurn {
    /// Rola.
    pub role: Role,
    /// Autor.
    pub author: Author,
    /// Treść (niezmienna po zapisie).
    pub content: TurnContent,
    /// Model/koszt (dla tur asystentki).
    pub usage: Option<ModelUsage>,
    /// Usłyszany prefiks, jeśli znany w chwili zapisu (można go dopisać później raz).
    pub heard_prefix: Option<HeardPrefix>,
}

impl NewTurn {
    /// Tura użytkownika z tekstem.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            author: Author::User,
            content: TurnContent::text(text),
            usage: None,
            heard_prefix: None,
        }
    }

    /// Tura asystentki (agentki `agent`) z tekstem.
    pub fn assistant(agent: &str, text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            author: Author::Agent {
                agent: AgentId::new(agent),
            },
            content: TurnContent::text(text),
            usage: None,
            heard_prefix: None,
        }
    }
}

/// Tura zapisana w historii.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Turn {
    /// Identyfikator.
    pub id: TurnId,
    /// Rodzic (`None` = początek rozmowy).
    pub parent: Option<TurnId>,
    /// Gałąź.
    pub branch: BranchId,
    /// Rola.
    pub role: Role,
    /// Autor.
    pub author: Author,
    /// Treść (niezmienna).
    pub content: TurnContent,
    /// Model/koszt.
    pub usage: Option<ModelUsage>,
    /// Czas zapisu.
    pub created_at: DateTime<Utc>,
    /// Usłyszany prefiks (fakt dopisywany raz; potem niezmienny).
    pub heard_prefix: Option<HeardPrefix>,
    /// Ukryta z widoku (flaga widoku; treść i audyt zostają).
    pub hidden: bool,
}

#[derive(Serialize)]
struct Fingerprint<'a> {
    id: TurnId,
    parent: Option<TurnId>,
    branch: BranchId,
    role: Role,
    author: &'a Author,
    content: &'a TurnContent,
    usage: &'a Option<ModelUsage>,
    created_at: &'a DateTime<Utc>,
}

impl Turn {
    /// Usłyszany tekst (prefiks `content.text`), jeśli prefiks zapisano.
    pub fn heard_text(&self) -> Option<&str> {
        let chars = self.heard_prefix?.chars;
        let end = self
            .content
            .text
            .char_indices()
            .nth(chars)
            .map_or(self.content.text.len(), |(i, _)| i);
        self.content.text.get(..end)
    }

    /// Kanoniczne bajty niezmiennej części tury (bez `heard_prefix` i `hidden`) — do dowodu
    /// append-only: te bajty nie mogą się zmienić po zapisie.
    pub fn fingerprint(&self) -> Vec<u8> {
        serde_json::to_vec(&Fingerprint {
            id: self.id,
            parent: self.parent,
            branch: self.branch,
            role: self.role,
            author: &self.author,
            content: &self.content,
            usage: &self.usage,
            created_at: &self.created_at,
        })
        .unwrap_or_default()
    }
}

/// Wspólna walidacja nowej tury (dzielona przez `-impl` i `-fake`).
pub fn validate_new_turn(turn: &NewTurn) -> Result<(), SessionError> {
    if turn.content.is_empty() {
        return Err(SessionError::EmptyTurn);
    }
    if let Some(prefix) = turn.heard_prefix {
        validate_heard_prefix(turn.role, &turn.content, prefix)?;
    }
    Ok(())
}

/// Usłyszany prefiks: tylko dla tury asystentki i nie dłuższy niż jej tekst.
pub fn validate_heard_prefix(
    role: Role,
    content: &TurnContent,
    prefix: HeardPrefix,
) -> Result<(), SessionError> {
    if role != Role::Assistant {
        return Err(SessionError::InvalidHeardPrefix {
            reason: "prefiks dotyczy wyłącznie tur asystentki".into(),
        });
    }
    let total = content.text.chars().count();
    if prefix.chars > total {
        return Err(SessionError::InvalidHeardPrefix {
            reason: format!("prefiks {} > długość tekstu {total}", prefix.chars),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "turn_tests.rs"]
mod tests;
