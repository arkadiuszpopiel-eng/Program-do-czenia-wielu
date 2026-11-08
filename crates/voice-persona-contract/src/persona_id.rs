//! Identyfikator persony (agentki) — głos idzie za personą (docs/PERSONAS.md §1).
//!
//! Jedno źródło prawdy: [`personas_contract::PersonaId`] (re-eksportowany z tego crate'a dla
//! wygody modułów głosu). Walidacja formatu identyfikatora z zewnątrz (konfiguracja, Kreator
//! agentów) — [`parse_persona_id`].

pub use personas_contract::PersonaId;

use crate::PersonaError;

/// Identyfikator persony po walidacji formatu `[a-z][a-z0-9-]{1,31}` (bez końcowego `-`;
/// reguła [`personas_contract::is_valid_id`]).
pub fn parse_persona_id(id: &str) -> Result<PersonaId, PersonaError> {
    PersonaId::parse(id).ok_or_else(|| PersonaError::UnknownPersona { id: id.to_owned() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_format() {
        assert_eq!(parse_persona_id("delta").unwrap(), PersonaId::delta());
        assert!(parse_persona_id("Delta").is_err());
        assert!(parse_persona_id("").is_err());
        assert!(parse_persona_id("a b").is_err());
        assert_eq!(
            parse_persona_id("x").unwrap_err(),
            PersonaError::UnknownPersona { id: "x".into() }
        );
        assert_eq!(PersonaId::builtin().len(), 4);
        assert_eq!(PersonaId::gama().to_string(), "gama");
        assert_eq!(
            serde_json::to_string(&PersonaId::beta()).unwrap(),
            "\"beta\""
        );
    }
}
