//! Kontrakt modułu-wzorca „echo” (docs/PLAN.md §3.2). Pokazuje, co zawiera każdy `-contract`:
//! trait, typy, nazwy zdarzeń i współdzielony test kontraktowy.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use async_trait::async_trait;
use core_bus_contract::EventKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

/// Rodzaj zdarzenia publikowanego po każdym udanym `echo` (konwencja `<moduł>.<obiekt>.<czynność>`).
pub const EVENT_ECHO_CALLED: &str = "example.echo.called";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn echo_called_kind() -> EventKind {
    EventKind::Custom(EVENT_ECHO_CALLED.to_owned())
}

/// Maksymalna długość wejścia (znaki); większe → `EchoError::TooLong`.
pub const MAX_INPUT_CHARS: usize = 4096;

/// Odpowiedź echa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EchoReply {
    /// Tekst wejściowy bez zmian.
    pub text: String,
    /// Liczba znaków (nie bajtów).
    pub chars: usize,
    /// Numer kolejny wywołania w tej instancji (od 1, monotoniczny).
    pub seq: u64,
}

/// Błędy echa.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum EchoError {
    /// Puste wejście.
    #[error("puste wejście")]
    Empty,
    /// Wejście dłuższe niż `MAX_INPUT_CHARS`.
    #[error("wejście za długie: {chars} > {max}")]
    TooLong {
        /// Długość wejścia.
        chars: usize,
        /// Limit.
        max: usize,
    },
    /// Moduł nie jest uruchomiony.
    #[error("moduł nie jest uruchomiony")]
    NotStarted,
}

/// Kontrakt echa.
#[async_trait]
pub trait Echo: Send + Sync {
    /// Zwraca wejście bez zmian z numerem kolejnym; publikuje `example.echo.called`.
    async fn echo(&self, input: &str) -> Result<EchoReply, EchoError>;
}

/// Wspólna walidacja wejścia (dzielona przez `-impl` i `-fake`, żeby nie rozjechały się reguły).
pub fn validate_input(input: &str) -> Result<usize, EchoError> {
    let chars = input.chars().count();
    if chars == 0 {
        return Err(EchoError::Empty);
    }
    if chars > MAX_INPUT_CHARS {
        return Err(EchoError::TooLong {
            chars,
            max: MAX_INPUT_CHARS,
        });
    }
    Ok(chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_rules() {
        assert_eq!(validate_input("zażółć"), Ok(6));
        assert_eq!(validate_input(""), Err(EchoError::Empty));
        let long = "x".repeat(MAX_INPUT_CHARS + 1);
        assert!(matches!(
            validate_input(&long),
            Err(EchoError::TooLong { .. })
        ));
    }

    #[test]
    fn error_serializes_tagged() {
        let json = serde_json::to_value(EchoError::Empty).unwrap();
        assert_eq!(json, serde_json::json!({"error": "empty"}));
    }
}
