//! Identyfikator persony (agentki) — głos idzie za personą (docs/PERSONAS.md §1).

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::PersonaError;

/// Identyfikator persony: małe litery ASCII, cyfry i `-` (np. `alfa`, `delta`).
///
/// Cztery persony wbudowane mają konstruktory [`PersonaId::alfa`] … [`PersonaId::delta`];
/// persony z Kreatora agentów (PLAN §9.5) tworzy się przez [`PersonaId::new`].
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct PersonaId(String);

impl PersonaId {
    /// Tworzy identyfikator po walidacji formatu.
    pub fn new(id: impl Into<String>) -> Result<Self, PersonaError> {
        let id = id.into();
        let valid = !id.is_empty()
            && id.len() <= 32
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if valid {
            Ok(Self(id))
        } else {
            Err(PersonaError::UnknownPersona { id })
        }
    }

    /// Alfa (α) — Dyrygentka + Mówczyni.
    pub fn alfa() -> Self {
        Self("alfa".into())
    }

    /// Beta (β) — Strażniczka pamięci i organizacji.
    pub fn beta() -> Self {
        Self("beta".into())
    }

    /// Gama (γ) — Badaczka + Krytyczka.
    pub fn gama() -> Self {
        Self("gama".into())
    }

    /// Delta (δ) — Wykonawczyni + Koderka.
    pub fn delta() -> Self {
        Self("delta".into())
    }

    /// Cztery persony wbudowane w kolejności Alfa, Beta, Gama, Delta.
    pub fn builtin() -> [Self; 4] {
        [Self::alfa(), Self::beta(), Self::gama(), Self::delta()]
    }

    /// Tekst identyfikatora.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PersonaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_format() {
        assert_eq!(PersonaId::new("delta").unwrap(), PersonaId::delta());
        assert!(PersonaId::new("Delta").is_err());
        assert!(PersonaId::new("").is_err());
        assert!(PersonaId::new("a b").is_err());
        assert_eq!(PersonaId::builtin().len(), 4);
        assert_eq!(PersonaId::gama().to_string(), "gama");
    }
}
