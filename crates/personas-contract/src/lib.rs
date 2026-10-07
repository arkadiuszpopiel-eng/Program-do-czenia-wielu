//! Kontrakt modułu `personas` (docs/modules/personas/SPEC.md, docs/PERSONAS.md, PLAN §9.2).
//!
//! Persona = stała tożsamość (imię, glif, token koloru, charakter, biblia głosu); rola = zmienna
//! (zadania, narzędzia, uprawnienia). Obsada ról jest per sesja. Crate zawiera typy, wbudowany
//! katalog, trait [`Personas`], nazwy zdarzeń oraz **deterministyczną logikę współdzieloną przez
//! `-impl` i `-fake`**: walidację obsad, rozstrzyganie adresatki i parser poleceń obsady
//! (inne moduły, np. `voice-dialog`, mogą ich używać bez usługi).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod address;
mod builtin;
mod cast;
mod catalog;
mod command;
mod ids;
mod model;
mod prompt;
mod state;
mod text;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use async_trait::async_trait;
use core_bus_contract::{AgentId, Event, EventKind, Level, SessionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use address::{Mention, find_mentions, parse_addressee, resolve_addressee};
pub use builtin::{builtin_personas, builtin_roles, builtin_templates};
pub use cast::{Cast, CastDiff, CastError, CastWarning, Verifier};
pub use catalog::{Catalog, EXPORT_VERSION, PersonasExport};
pub use command::{CastCommand, apply_command, parse_cast_command};
pub use ids::{ColorToken, PersonaId, RoleId, TemplateId, is_valid_id};
pub use model::{
    Case, CastTemplate, FORBIDDEN_VOICE_WORDS, ModelError, NameForms, Persona, Role, VoiceBible,
};
pub use prompt::{
    COMMON_RULES, DEFAULT_PROMPT_TEMPLATE, PromptError, render_system_prompt, validate_template,
};
pub use state::PersonasState;
pub use text::{Token, fold, tokenize};

/// Zmiana obsady (Audyt: kto, skąd — UI / głos / Marszałek).
pub const EVENT_CAST_CHANGED: &str = "personas.cast.changed";
/// Nowy przydział roli (po jednym zdarzeniu na parę persona–rola).
pub const EVENT_ROLE_ASSIGNED: &str = "personas.role.assigned";
/// Dodano personę z Kreatora.
pub const EVENT_PERSONA_ADDED: &str = "personas.persona.added";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Źródło zmiany obsady.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOrigin {
    /// Ustawienia → Agentki → Obsada albo pasek sesji.
    Ui,
    /// Polecenie głosowe.
    Voice,
    /// Polecenie tekstowe (`/obsada`, composer).
    Text,
    /// Marszałek na polecenie użytkownika.
    Marshal,
    /// Program (np. przełączenie sesji w tryb głosowy).
    System,
    /// Import paczki `.alfa`.
    Import,
}

/// Wynik udanej zmiany obsady.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CastChange {
    /// Sesja.
    pub session: SessionId,
    /// Źródło zmiany.
    pub origin: ChangeOrigin,
    /// Obsada przed.
    pub before: Cast,
    /// Obsada po.
    pub after: Cast,
    /// Różnica.
    pub diff: CastDiff,
    /// Ostrzeżenia walidacji nowej obsady.
    pub warnings: Vec<CastWarning>,
}

impl CastChange {
    /// Zdarzenia magistrali dla tej zmiany (wspólne dla `-impl` i `-fake`).
    pub fn events(&self) -> Vec<Event> {
        let mut events = vec![
            Event::new(
                event_kind(EVENT_CAST_CHANGED),
                Level::Info,
                serde_json::json!({
                    "origin": self.origin,
                    "before": self.before,
                    "after": self.after,
                    "assigned": self.diff.assigned,
                    "removed": self.diff.removed,
                    "warnings": self.warnings,
                }),
            )
            .with_session(self.session.clone()),
        ];
        for (persona, role) in &self.diff.assigned {
            events.push(
                Event::new(
                    event_kind(EVENT_ROLE_ASSIGNED),
                    Level::Info,
                    serde_json::json!({ "persona": persona, "role": role, "origin": self.origin }),
                )
                .with_session(self.session.clone())
                .with_agent(AgentId::new(persona.as_str())),
            );
        }
        events
    }
}

/// Zdarzenie dodania persony (Kreator).
pub fn persona_added_event(persona: &Persona) -> Event {
    Event::new(
        event_kind(EVENT_PERSONA_ADDED),
        Level::Info,
        serde_json::json!({ "persona": persona.id, "name": persona.name }),
    )
    .with_agent(AgentId::new(persona.id.as_str()))
}

/// Błędy usługi person.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum PersonasError {
    /// Niepoprawna obsada.
    #[error(transparent)]
    Cast(#[from] CastError),
    /// Niepoprawne dane persony/roli/szablonu.
    #[error(transparent)]
    Model(#[from] ModelError),
    /// Błąd szablonu promptu.
    #[error(transparent)]
    Prompt(#[from] PromptError),
    /// Nieznana persona.
    #[error("nieznana persona `{0}`")]
    UnknownPersona(PersonaId),
    /// Moduł nie jest uruchomiony (zmiany wymagają magistrali — muszą trafić do dziennika).
    #[error("moduł person nie jest uruchomiony")]
    NotStarted,
}

/// Usługa person i obsad. Zmiana obsady jest natychmiastowa (bez restartu sesji) i publikuje
/// `personas.cast.changed`. Persona nie zmienia obsady sama — `origin` wskazuje polecenie użytkownika.
#[async_trait]
pub trait Personas: Send + Sync {
    /// Wszystkie persony (wbudowane + własne).
    fn personas(&self) -> Vec<Persona>;

    /// Wszystkie role.
    fn roles(&self) -> Vec<Role>;

    /// Wszystkie szablony obsad.
    fn templates(&self) -> Vec<CastTemplate>;

    /// Obsada sesji; dla nowej sesji — z domyślnego szablonu (sesja tekstowa).
    fn cast(&self, session: &SessionId) -> Cast;

    /// Ustawia obsadę po walidacji; publikuje zdarzenia.
    async fn set_cast(
        &self,
        session: &SessionId,
        cast: Cast,
        origin: ChangeOrigin,
    ) -> Result<CastChange, PersonasError>;

    /// Przełącza sesję w tryb głosowy/tekstowy; bez Mówczyni rolę dostaje Dyrygentka.
    async fn set_voice(
        &self,
        session: &SessionId,
        voice: bool,
        origin: ChangeOrigin,
    ) -> Result<CastChange, PersonasError>;

    /// Rozpoznaje i stosuje polecenie obsady z tekstu; `Ok(None)`, gdy tekst nie jest poleceniem.
    async fn apply_command(
        &self,
        session: &SessionId,
        text: &str,
        origin: ChangeOrigin,
    ) -> Result<Option<CastChange>, PersonasError>;

    /// Adresatka wypowiedzi: imię wygrywa, inaczej Dyrygentka obsady sesji.
    fn resolve_addressee(&self, session: &SessionId, text: &str) -> PersonaId;

    /// Prompt systemowy persony z jej rolami w sesji.
    fn system_prompt(
        &self,
        session: &SessionId,
        persona: &PersonaId,
    ) -> Result<String, PersonasError>;

    /// Kreator: dodaje własną personę (walidacja, unikalność imienia i form).
    async fn add_persona(&self, persona: Persona) -> Result<(), PersonasError>;

    /// Dodaje własną rolę.
    async fn add_role(&self, role: Role) -> Result<(), PersonasError>;

    /// Dodaje własny szablon obsady.
    async fn add_template(&self, template: CastTemplate) -> Result<(), PersonasError>;

    /// Eksport do `.alfa` (elementy własne + obsady sesji).
    fn export(&self) -> PersonasExport;
}
