//! Klucz konfiguracji jako ścieżka kropkowa.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Błąd klucza.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum KeyError {
    /// Klucz pusty lub segment niepoprawny.
    #[error(
        "niepoprawny klucz konfiguracji `{0}` (oczekiwano `seg.seg.seg`, segmenty `[a-z0-9_]+`)"
    )]
    Invalid(String),
    /// Klucz wygląda na sekret — sekrety nie wchodzą do konfiguracji (SPEC core-config).
    #[error("klucz `{0}` wygląda na sekret; sekrety trzyma wyłącznie Credential Manager")]
    LooksLikeSecret(String),
}

/// Klucz konfiguracji, np. `voice.stt.engine`. Segmenty `[a-z0-9_]+` rozdzielone kropkami.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct ConfigKey(String);

/// Ostatni segment równy jednemu z tych słów lub kończący się na `_<słowo>` oznacza sekret.
const SECRET_WORDS: [&str; 4] = ["token", "secret", "password", "passphrase"];

impl ConfigKey {
    /// Tworzy klucz z walidacją składni i odrzuceniem nazw sekretów.
    pub fn new(path: impl Into<String>) -> Result<Self, KeyError> {
        let path = path.into();
        let valid = !path.is_empty()
            && path.split('.').all(|seg| {
                !seg.is_empty()
                    && seg
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            });
        if !valid {
            return Err(KeyError::Invalid(path));
        }
        let last = path.rsplit('.').next().unwrap_or_default();
        let is_secret = last.ends_with("_key")
            || SECRET_WORDS
                .iter()
                .any(|w| last == *w || last.ends_with(&format!("_{w}")));
        if is_secret {
            return Err(KeyError::LooksLikeSecret(path));
        }
        Ok(Self(path))
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Segmenty klucza.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('.')
    }

    /// Czy klucz leży pod prefiksem (`"voice"` pasuje do `voice.stt.engine`, nie do `voices.x`).
    pub fn has_prefix(&self, prefix: &str) -> bool {
        prefix.is_empty()
            || self.0 == prefix
            || self
                .0
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('.'))
    }
}

impl FromStr for ConfigKey {
    type Err = KeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl<'de> Deserialize<'de> for ConfigKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_keys() {
        let k = ConfigKey::new("voice.stt.engine").unwrap();
        assert_eq!(k.segments().count(), 3);
        assert!(k.has_prefix("voice"));
        assert!(k.has_prefix("voice.stt"));
        assert!(k.has_prefix(""));
        assert!(!k.has_prefix("voic"));
        assert!(!k.has_prefix("voice.stt.engine.x"));
    }

    #[test]
    fn invalid_and_secret_keys() {
        for bad in ["", ".", "a..b", "Voice.stt", "a-b", "a.", ".a"] {
            assert!(
                matches!(ConfigKey::new(bad), Err(KeyError::Invalid(_))),
                "{bad}"
            );
        }
        for secret in [
            "providers.anthropic.api_key",
            "x.token",
            "y.password",
            "z.oauth_secret",
        ] {
            assert!(
                matches!(ConfigKey::new(secret), Err(KeyError::LooksLikeSecret(_))),
                "{secret}"
            );
        }
    }

    #[test]
    fn serde_rejects_invalid() {
        assert!(serde_json::from_str::<ConfigKey>("\"A.B\"").is_err());
        let ok: ConfigKey = serde_json::from_str("\"a.b\"").unwrap();
        assert_eq!(ok.as_str(), "a.b");
    }
}
