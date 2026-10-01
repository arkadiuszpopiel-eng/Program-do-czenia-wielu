//! Identyfikatory zadań i wysłań.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Najdłuższy identyfikator zadania.
pub const MAX_TASK_ID_LEN: usize = 96;

/// Identyfikator zadania — nadaje zgłaszająca (np. `plan-7/zbierz-faktury`); unikalny w schedulerze.
/// Dozwolone znaki: `[A-Za-z0-9._:/-]`, długość 1–96.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct TaskId(pub String);

impl TaskId {
    /// Identyfikator z tekstu (bez walidacji; patrz [`TaskId::is_valid`]).
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Czy identyfikator ma poprawną postać.
    pub fn is_valid(&self) -> bool {
        !self.0.is_empty()
            && self.0.len() <= MAX_TASK_ID_LEN
            && self
                .0
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '-'))
    }

    /// Identyfikator podzadania (`<rodzic>/<sufiks>`).
    pub fn child(&self, suffix: &str) -> Self {
        Self(format!("{}/{suffix}", self.0))
    }
}

impl From<&str> for TaskId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for TaskId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Identyfikator wysłania zadania do wykonawczyni (jedna próba wykonania od przydziału do
/// zwrotu). Spóźnione raporty ze starego wysłania są ignorowane.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct DispatchId(pub u64);

impl fmt::Display for DispatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "d{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validity() {
        assert!(TaskId::new("plan-7/zbierz_faktury.v2:a").is_valid());
        assert!(!TaskId::new("").is_valid());
        assert!(!TaskId::new("ze spacją").is_valid());
        assert!(!TaskId::new("ł").is_valid());
        assert!(!TaskId::new("x".repeat(MAX_TASK_ID_LEN + 1)).is_valid());
        assert_eq!(TaskId::from("a").child("b").as_str(), "a/b");
        assert_eq!(DispatchId(3).to_string(), "d3");
        assert_eq!(TaskId::from("a").to_string(), "a");
    }
}
