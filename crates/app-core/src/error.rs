//! Błąd komend `AppCore` — kod + komunikat po polsku (UI pokazuje komunikat; EN wg kodu).

use serde::{Deserialize, Serialize};

/// Kod błędu komendy (stabilny, do testów i ewentualnej logiki UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Obiekt (sesja, tura, konto, artefakt…) nie istnieje.
    NotFound,
    /// Niepoprawne dane wejściowe komendy.
    InvalidInput,
    /// Funkcja wymaga modułu, który nie jest jeszcze podłączony (router, transfer, głos, Broker…).
    Unavailable,
    /// Operacja zabroniona (lista dozwolonych, polityka Jądra, reguła AltGr).
    Forbidden,
    /// Błąd magazynu danych (SQLCipher, pliki konfiguracji, dziennik kosztów).
    Storage,
    /// Magazyn sekretów (Credential Manager) niedostępny.
    Secrets,
    /// Błąd dostawcy modeli lub konta.
    Provider,
    /// Błąd wewnętrzny składania modułów.
    Internal,
}

impl ErrorCode {
    /// Ogólny komunikat angielski (szczegóły modułów są po polsku).
    pub fn english(self) -> &'static str {
        match self {
            Self::NotFound => "Not found.",
            Self::InvalidInput => "Invalid input.",
            Self::Unavailable => "This feature becomes available once its module is connected.",
            Self::Forbidden => "Operation not allowed.",
            Self::Storage => "Data storage error.",
            Self::Secrets => "Windows Credential Manager is unavailable.",
            Self::Provider => "Model provider or account error.",
            Self::Internal => "Internal error.",
        }
    }
}

/// Błąd komendy: kod i komunikat PL gotowy do pokazania użytkownikowi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    /// Kod.
    pub code: ErrorCode,
    /// Komunikat po polsku (bez sekretów).
    pub message: String,
}

impl AppError {
    /// Nowy błąd.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Obiekt nie istnieje.
    pub fn not_found(what: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, what)
    }

    /// Niepoprawne dane.
    pub fn invalid(what: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, what)
    }

    /// Operacja zabroniona.
    pub fn forbidden(what: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, what)
    }

    /// Błąd magazynu.
    pub fn storage(what: impl std::fmt::Display) -> Self {
        Self::new(ErrorCode::Storage, format!("Błąd zapisu danych: {what}"))
    }

    /// Błąd wewnętrzny.
    pub fn internal(what: impl std::fmt::Display) -> Self {
        Self::new(ErrorCode::Internal, format!("Błąd wewnętrzny: {what}"))
    }

    /// Funkcja dostępna po podłączeniu modułu `module` (port z domyślną implementacją).
    pub fn unavailable(feature: &str, module: &str) -> Self {
        Self::new(
            ErrorCode::Unavailable,
            format!("{feature}: funkcja dostępna po podłączeniu modułu {module}."),
        )
    }

    /// Komunikat w języku interfejsu (`pl` — pełny; `en` — ogólny wg kodu + szczegóły PL).
    pub fn localized(&self, locale: &str) -> String {
        if locale == "en" {
            format!("{} ({})", self.code.english(), self.message)
        } else {
            self.message.clone()
        }
    }
}

impl From<sessions_contract::SessionError> for AppError {
    fn from(e: sessions_contract::SessionError) -> Self {
        use sessions_contract::SessionError as E;
        match e {
            E::NotFound { .. } | E::TurnNotFound { .. } => Self::not_found(e.to_string()),
            E::Vault { .. } => Self::new(ErrorCode::Secrets, e.to_string()),
            E::Storage { .. } => Self::storage(e),
            other => Self::invalid(other.to_string()),
        }
    }
}

impl From<accounts_hub_contract::AccountsError> for AppError {
    fn from(e: accounts_hub_contract::AccountsError) -> Self {
        use accounts_hub_contract::AccountsError as E;
        match e {
            E::UnknownAccount(_) | E::UnknownProvider(_) => Self::not_found(e.to_string()),
            E::ProviderForbidden(_) | E::NotPermitted(_) => Self::forbidden(e.to_string()),
            E::Secret(_) | E::SecretMissing(_) => Self::new(ErrorCode::Secrets, e.to_string()),
            E::Persist(_) => Self::storage(e),
            E::InvalidInput(_) | E::Wizard(_) | E::Catalog(_) => Self::invalid(e.to_string()),
            other => Self::new(ErrorCode::Provider, other.to_string()),
        }
    }
}

impl From<core_config_contract::ConfigError> for AppError {
    fn from(e: core_config_contract::ConfigError) -> Self {
        use core_config_contract::ConfigError as E;
        match e {
            E::KernelPolicy(_) => Self::forbidden(e.to_string()),
            E::SchemaViolation { .. } | E::UnknownKey(_) => Self::invalid(e.to_string()),
            E::Persist(_) => Self::storage(e),
        }
    }
}

impl From<artifacts_contract::ArtifactError> for AppError {
    fn from(e: artifacts_contract::ArtifactError) -> Self {
        use artifacts_contract::ArtifactError as E;
        match e {
            E::NotFound { .. } | E::VersionNotFound { .. } | E::FileNotFound { .. } => {
                Self::not_found(e.to_string())
            }
            E::Storage { .. } => Self::storage(e),
            other => Self::invalid(other.to_string()),
        }
    }
}

impl From<personas_contract::PersonasError> for AppError {
    fn from(e: personas_contract::PersonasError) -> Self {
        match e {
            personas_contract::PersonasError::NotStarted => Self::internal(e),
            other => Self::invalid(other.to_string()),
        }
    }
}

impl From<memory_contract::MemoryError> for AppError {
    fn from(e: memory_contract::MemoryError) -> Self {
        Self::invalid(e.to_string())
    }
}

impl From<search_contract::SearchError> for AppError {
    fn from(e: search_contract::SearchError) -> Self {
        Self::storage(e)
    }
}

impl From<cost_meter_contract::CostError> for AppError {
    fn from(e: cost_meter_contract::CostError) -> Self {
        Self::invalid(e.to_string())
    }
}

impl From<core_config_contract::KeyError> for AppError {
    fn from(e: core_config_contract::KeyError) -> Self {
        Self::invalid(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_names_module_and_localizes() {
        let e = AppError::unavailable("Import paczki", "transfer");
        assert_eq!(e.code, ErrorCode::Unavailable);
        assert!(e.message.contains("modułu transfer"));
        assert!(e.localized("en").starts_with("This feature"));
        assert_eq!(e.localized("pl"), e.message);
    }

    #[test]
    fn session_errors_map_to_codes() {
        let nf: AppError = sessions_contract::SessionError::NotFound {
            id: sessions_contract::SessionId::new("x"),
        }
        .into();
        assert_eq!(nf.code, ErrorCode::NotFound);
        let bad: AppError = sessions_contract::SessionError::EmptyTurn.into();
        assert_eq!(bad.code, ErrorCode::InvalidInput);
    }
}
