//! Błąd komend `AppCore` — kod + komunikat po polsku (UI pokazuje komunikat; EN wg kodu)
//! + opcjonalne szczegóły do odczytu maszynowego (`details`, zadanie B.3 planu v6).

use serde::{Deserialize, Serialize};

pub use crate::details::{DetailValue, ErrorDetails};

/// Kod błędu komendy (stabilny, do testów i ewentualnej logiki UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error, schemars::JsonSchema,
)]
#[error("{message}")]
pub struct AppError {
    /// Kod.
    pub code: ErrorCode,
    /// Komunikat po polsku (bez sekretów).
    pub message: String,
    /// Szczegóły do odczytu maszynowego: stałe klucze (`kind`, `id`, `field`, `reason`, `module`,
    /// `provider`, `retry_after_s`), bez sekretów i treści użytkownika. Puste — pole pominięte
    /// na łączu (kształt sprzed B.3).
    #[serde(default, skip_serializing_if = "ErrorDetails::is_empty")]
    pub details: ErrorDetails,
}

impl AppError {
    /// Nowy błąd.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: ErrorDetails::default(),
        }
    }

    /// Dokłada szczegół (wpis odrzucony przez filtr sekretów jest pomijany — patrz `details.rs`).
    #[must_use]
    pub fn with(mut self, key: &str, value: impl Into<DetailValue>) -> Self {
        self.details.insert(key, value);
        self
    }

    /// Obiekt rodzaju `kind` o identyfikatorze `id` nie istnieje.
    pub fn missing(kind: &str, id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::not_found(message)
            .with("kind", kind)
            .with("id", id.into())
    }

    /// Niepoprawna wartość argumentu albo pola `field`.
    pub fn invalid_field(field: &str, message: impl Into<String>) -> Self {
        Self::invalid(message).with("field", field)
    }

    /// Dostawca modeli chwilowo odmawia (limit zapytań, przeciążenie); `retry_after_s` — po ilu
    /// sekundach ponowić, jeśli dostawca to podał.
    pub fn provider_busy(
        provider: &str,
        retry_after_s: Option<u64>,
        message: impl Into<String>,
    ) -> Self {
        let error = Self::new(ErrorCode::Provider, message)
            .with("provider", provider)
            .with("reason", "rate_limited");
        match retry_after_s {
            Some(seconds) => error.with("retry_after_s", seconds),
            None => error,
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
        .with("module", module)
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
        let text = e.to_string();
        match e {
            E::NotFound { id } => Self::missing("session", id.to_string(), text),
            E::TurnNotFound { turn } => Self::missing("turn", turn.0.to_string(), text),
            E::Vault { .. } => Self::new(ErrorCode::Secrets, e.to_string()),
            E::Storage { .. } => Self::storage(e),
            other => Self::invalid(other.to_string()),
        }
    }
}

impl From<accounts_hub_contract::AccountsError> for AppError {
    fn from(e: accounts_hub_contract::AccountsError) -> Self {
        use accounts_hub_contract::AccountsError as E;
        let text = e.to_string();
        match e {
            E::UnknownAccount(id) => Self::missing("account", id.to_string(), text),
            E::UnknownProvider(id) => Self::missing("provider", id.to_string(), text),
            E::ProviderForbidden(id) => Self::forbidden(text).with("provider", id.to_string()),
            E::AccountDisabled(id) => Self::new(ErrorCode::Provider, text)
                .with("kind", "account")
                .with("id", id.to_string())
                .with("reason", "disabled"),
            E::NotPermitted(_) => Self::forbidden(text),
            E::Secret(_) | E::SecretMissing(_) => Self::new(ErrorCode::Secrets, e.to_string()),
            E::Persist(_) => Self::storage(e),
            E::InvalidInput(_) | E::Wizard(_) | E::Catalog(_) => Self::invalid(e.to_string()),
        }
    }
}

impl From<core_config_contract::ConfigError> for AppError {
    fn from(e: core_config_contract::ConfigError) -> Self {
        use core_config_contract::ConfigError as E;
        let text = e.to_string();
        match e {
            E::KernelPolicy(key) => Self::forbidden(text).with("field", key.to_string()),
            E::SchemaViolation { key, .. } | E::UnknownKey(key) => {
                Self::invalid_field(&key.to_string(), text)
            }
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
            personas_contract::PersonasError::Storage(why) => Self::storage(why),
            personas_contract::PersonasError::NotStarted => Self::internal(e),
            other => Self::invalid(other.to_string()),
        }
    }
}

impl From<memory_contract::MemoryError> for AppError {
    fn from(e: memory_contract::MemoryError) -> Self {
        use memory_contract::MemoryError as E;
        let text = format!("Pamięć: {e}");
        match e {
            E::NotFound { id } => Self::missing("memory", id.to_string(), text),
            E::Forbidden { .. }
            | E::PrivateSource { .. }
            | E::UntrustedCannotPromote
            | E::UntrustedAutoRemember => Self::forbidden(text),
            E::Storage { .. } => Self::storage(text),
            _ => Self::invalid(text),
        }
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
