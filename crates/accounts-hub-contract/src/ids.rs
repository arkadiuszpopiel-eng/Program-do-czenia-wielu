//! Identyfikatory: dostawca, konto, model, nazwa sekretu (wszystkie walidowane przy tworzeniu).

use std::fmt;

use compliance_contract::is_kebab;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::AccountsError;

macro_rules! validated_id {
    ($(#[$doc:meta])* $name:ident, $check:expr, $what:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Tworzy identyfikator po walidacji.
            pub fn new(value: impl Into<String>) -> Result<Self, AccountsError> {
                let value = value.into();
                let check: fn(&str) -> bool = $check;
                if check(&value) {
                    Ok(Self(value))
                } else {
                    Err(AccountsError::InvalidInput(format!(
                        concat!("niepoprawny ", $what, ": `{}`"),
                        value
                    )))
                }
            }

            /// Widok tekstowy.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(d)?;
                Self::new(raw).map_err(serde::de::Error::custom)
            }
        }
    };
}

validated_id!(
    /// Identyfikator dostawcy = nazwa pliku katalogu (kebab-case).
    ProviderId,
    is_kebab,
    "identyfikator dostawcy"
);

validated_id!(
    /// Identyfikator konta (`acc-…`, kebab-case).
    AccountId,
    |s| s.starts_with("acc-") && is_kebab(s),
    "identyfikator konta"
);

validated_id!(
    /// Identyfikator modelu u dostawcy (np. `claude-opus-5-5`); `*` = cena domyślna w cenniku.
    ModelId,
    |s| !s.is_empty() && s.len() <= 200 && !s.chars().any(|c| c.is_control() || c.is_whitespace()),
    "identyfikator modelu"
);

validated_id!(
    /// Nazwa sekretu w magazynie (bez prefiksu `Alfa/`): `[A-Za-z0-9._/-]`, bez `..`, ≤ 200 znaków.
    SecretName,
    is_secret_name,
    "nazwa sekretu"
);

fn is_secret_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 200
        && !s.starts_with('/')
        && !s.ends_with('/')
        && !s.contains("..")
        && !s.contains("//")
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
}

impl SecretName {
    /// Nazwa sekretu konta: `accounts/<id>`.
    pub fn for_account(id: &AccountId) -> Self {
        Self(format!("accounts/{}", id.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        assert!(ProviderId::new("custom-openai-compatible").is_ok());
        assert!(ProviderId::new("Anthropic").is_err());
        assert!(AccountId::new("acc-1").is_ok());
        assert!(AccountId::new("1").is_err());
        assert!(ModelId::new("models/gemini-3.0-pro").is_ok());
        assert!(ModelId::new("a b").is_err());
        let acc = AccountId::new("acc-7").unwrap();
        assert_eq!(SecretName::for_account(&acc).as_str(), "accounts/acc-7");
        for bad in ["", "/x", "x/", "a/../b", "a//b", "a b", "ł"] {
            assert!(SecretName::new(bad).is_err(), "{bad}");
        }
        let parsed: ProviderId = serde_json::from_str("\"xai\"").unwrap();
        assert_eq!(parsed.as_str(), "xai");
        assert!(serde_json::from_str::<ProviderId>("\"X AI\"").is_err());
    }
}
