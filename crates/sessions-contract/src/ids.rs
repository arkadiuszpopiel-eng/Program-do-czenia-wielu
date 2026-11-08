//! Identyfikatory sesji, tur, gałęzi i projektów.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use core_bus_contract::{AgentId, SessionId};

macro_rules! seq_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
        )]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "{}"), self.0)
            }
        }
    };
}

seq_id!(
    /// Identyfikator tury: numer kolejny w obrębie sesji (od 1, rosnący, nigdy nieużyty ponownie).
    TurnId,
    "t"
);
seq_id!(
    /// Identyfikator gałęzi w obrębie sesji (od 1; gałąź 1 = pierwsza linia rozmowy).
    BranchId,
    "b"
);

/// Identyfikator projektu (folderu sesji w panelu Sesje).
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ProjectId(pub String);

impl ProjectId {
    /// Tworzy identyfikator projektu.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_display_and_serialize_transparently() {
        assert_eq!(TurnId(7).to_string(), "t7");
        assert_eq!(BranchId(2).to_string(), "b2");
        assert_eq!(serde_json::to_string(&TurnId(7)).unwrap(), "7");
        assert_eq!(ProjectId::new("praca").to_string(), "praca");
    }
}
