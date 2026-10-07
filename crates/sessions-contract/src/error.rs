//! Błędy modułu `sessions` (serializowalne — trafiają do UI przez IPC).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{SessionId, TurnId};

/// Błędy sesji.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum SessionError {
    /// Sesja nie istnieje (albo została usunięta).
    #[error("sesja {id} nie istnieje")]
    NotFound {
        /// Identyfikator sesji.
        id: SessionId,
    },
    /// Sesja o tym identyfikatorze już istnieje (import z zachowaniem identyfikatora).
    #[error("sesja {id} już istnieje")]
    AlreadyExists {
        /// Identyfikator sesji.
        id: SessionId,
    },
    /// Tura nie istnieje w tej sesji.
    #[error("tura {turn} nie istnieje w tej sesji")]
    TurnNotFound {
        /// Identyfikator tury.
        turn: TurnId,
    },
    /// Tura ma już kontynuację; nowa odpowiedź od tego miejsca = nowa gałąź (`fork_from`).
    #[error("tura {turn} ma już kontynuację — użyj fork_from (nowa gałąź)")]
    NotALeaf {
        /// Tura, do której próbowano dopisać.
        turn: TurnId,
    },
    /// Sesja ma już pierwszą turę; alternatywny początek rozmowy tylko przez `fork_from`.
    #[error("sesja ma już pierwszą turę — nowy początek tylko przez fork_from")]
    RootExists,
    /// Tura bez tekstu i bez bloków.
    #[error("pusta tura")]
    EmptyTurn,
    /// Usłyszany prefiks niezgodny z turą.
    #[error("nieprawidłowy usłyszany prefiks: {reason}")]
    InvalidHeardPrefix {
        /// Opis problemu.
        reason: String,
    },
    /// Usłyszany prefiks tej tury jest już zapisany (fakt append-only).
    #[error("usłyszany prefiks tury {turn} jest już zapisany")]
    HeardPrefixAlreadyRecorded {
        /// Tura.
        turn: TurnId,
    },
    /// Nieprawidłowe dane wejściowe.
    #[error("nieprawidłowe dane: {reason}")]
    Invalid {
        /// Opis problemu.
        reason: String,
    },
    /// Sejf kluczy niedostępny albo odmówił operacji.
    #[error("sejf kluczy: {reason}")]
    Vault {
        /// Opis problemu (bez materiału klucza).
        reason: String,
    },
    /// Błąd magazynu (SQLite/SQLCipher/we-wy).
    #[error("magazyn: {reason}")]
    Storage {
        /// Opis problemu.
        reason: String,
    },
}

impl SessionError {
    /// Skrót: błąd magazynu z dowolnego błędu.
    pub fn storage(err: impl std::fmt::Display) -> Self {
        SessionError::Storage {
            reason: err.to_string(),
        }
    }

    /// Skrót: nieprawidłowe dane.
    pub fn invalid(reason: impl Into<String>) -> Self {
        SessionError::Invalid {
            reason: reason.into(),
        }
    }
}

impl From<crate::vault::VaultError> for SessionError {
    fn from(err: crate::vault::VaultError) -> Self {
        SessionError::Vault {
            reason: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_serialize_tagged() {
        let json = serde_json::to_value(SessionError::NotALeaf { turn: TurnId(3) }).unwrap();
        assert_eq!(json, serde_json::json!({"error": "not_a_leaf", "turn": 3}));
        assert_eq!(
            SessionError::storage("dysk").to_string(),
            "magazyn: dysk".to_owned()
        );
    }
}
