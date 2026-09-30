//! Identyfikatory: persona, rola, szablon obsady, token koloru.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Tworzy identyfikator z tekstu (bez walidacji; patrz `is_valid_id`).
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Widok tekstowy.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// Identyfikator persony (`alfa`, `beta`, `gama`, `delta` albo własny z Kreatora).
    PersonaId
);
string_id!(
    /// Identyfikator roli (`conductor`, `speaker`, … albo własny).
    RoleId
);
string_id!(
    /// Identyfikator szablonu obsady (`standard`, `solo`, `coding`, `research` albo własny).
    TemplateId
);
string_id!(
    /// Nazwa tokenu koloru z `packages/ui-kit` (np. `color.agent.alfa`) — nigdy wartość hex.
    ColorToken
);

/// Poprawny identyfikator: `[a-z][a-z0-9-]{1,31}` (bez końcowego `-`).
pub fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    first_ok
        && (2..=32).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.ends_with('-')
}

impl PersonaId {
    /// Alfa (α).
    pub fn alfa() -> Self {
        Self::from("alfa")
    }
    /// Beta (β).
    pub fn beta() -> Self {
        Self::from("beta")
    }
    /// Gama (γ).
    pub fn gama() -> Self {
        Self::from("gama")
    }
    /// Delta (δ).
    pub fn delta() -> Self {
        Self::from("delta")
    }
}

impl RoleId {
    /// Dyrygentka — koordynuje, odpowiada, gdy nikt nie jest wywołany po imieniu.
    pub fn conductor() -> Self {
        Self::from("conductor")
    }
    /// Mówczyni — prowadzi rozmowę głosową.
    pub fn speaker() -> Self {
        Self::from("speaker")
    }
    /// Myślicielka / Planistka.
    pub fn thinker() -> Self {
        Self::from("thinker")
    }
    /// Wykonawczyni / Operatorka komputera.
    pub fn operator() -> Self {
        Self::from("operator")
    }
    /// Koderka.
    pub fn coder() -> Self {
        Self::from("coder")
    }
    /// Krytyczka / Weryfikatorka (tylko odczyt).
    pub fn critic() -> Self {
        Self::from("critic")
    }
    /// Badaczka (niezaufane źródła w izolacji).
    pub fn researcher() -> Self {
        Self::from("researcher")
    }
    /// Strażniczka pamięci / organizacji.
    pub fn keeper() -> Self {
        Self::from("keeper")
    }
    /// Pisarka / Tłumaczka.
    pub fn writer() -> Self {
        Self::from("writer")
    }
}

impl TemplateId {
    /// Szablon „Standard” (domyślny).
    pub fn standard() -> Self {
        Self::from("standard")
    }
    /// Szablon „Solo” — jedna agentka, wszystkie role.
    pub fn solo() -> Self {
        Self::from("solo")
    }
    /// Szablon „Kodowanie”.
    pub fn coding() -> Self {
        Self::from("coding")
    }
    /// Szablon „Badania”.
    pub fn research() -> Self {
        Self::from("research")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_rules() {
        for ok in ["alfa", "a1", "moja-agentka", "x-2"] {
            assert!(is_valid_id(ok), "{ok}");
        }
        for bad in ["", "a", "Alfa", "1a", "a_b", "a-", "ą", &"a".repeat(33)] {
            assert!(!is_valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn ids_serialize_as_plain_strings() {
        assert_eq!(
            serde_json::to_string(&PersonaId::gama()).ok().as_deref(),
            Some("\"gama\"")
        );
        assert_eq!(RoleId::critic().to_string(), "critic");
    }
}
