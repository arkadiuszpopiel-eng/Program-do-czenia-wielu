//! Klucz API z redakcją i zerowaniem pamięci. Klucze pochodzą wyłącznie z Windows Credential
//! Manager (przez `accounts-hub`); nigdy z plików konfiguracji, logów ani eksportu `.alfa`.

use std::fmt;

use zeroize::Zeroizing;

/// Tekst zastępujący klucz w `Debug`/`Display`.
pub const REDACTED_KEY: &str = "[REDACTED]";

/// Klucz API. `Debug` i `Display` nigdy nie ujawniają wartości; pamięć jest zerowana przy zwolnieniu.
/// Typ celowo nie implementuje `Serialize`.
///
/// ```
/// use providers_contract::ApiKey;
/// let key = ApiKey::new("sk-ant-tajne");
/// assert_eq!(format!("{key:?}"), "ApiKey([REDACTED])");
/// assert_eq!(key.to_string(), "[REDACTED]");
/// assert_eq!(key.expose(), "sk-ant-tajne");
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(Zeroizing<String>);

impl ApiKey {
    /// Opakowuje klucz.
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }

    /// Wartość klucza — wyłącznie do ustawienia nagłówka żądania (oznaczonego jako wrażliwy).
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    /// Czy klucz jest pusty.
    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }

    /// Zastępuje wystąpienia klucza w tekście (np. w treści błędu dostawcy).
    pub fn redact_in(&self, text: &str) -> String {
        if self.is_empty() {
            text.to_owned()
        } else {
            text.replace(self.expose(), REDACTED_KEY)
        }
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApiKey({REDACTED_KEY})")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED_KEY)
    }
}

/// Źródło klucza pobieranego **w momencie wywołania** (rotacja bez restartu, PLAN §5.6).
/// Implementuje `accounts-hub` (Credential Manager); testy — [`StaticKey`].
pub trait SecretSource: Send + Sync {
    /// Aktualny klucz; `None` = dostawca nieskonfigurowany (Router go pomija).
    fn api_key(&self) -> Option<ApiKey>;
}

/// Stały klucz (testy, fixture'y) albo jego brak.
#[derive(Clone, Debug, Default)]
pub struct StaticKey(pub Option<ApiKey>);

impl StaticKey {
    /// Źródło zwracające zawsze ten klucz.
    pub fn new(value: impl Into<String>) -> Self {
        Self(Some(ApiKey::new(value)))
    }

    /// Źródło bez klucza.
    pub fn none() -> Self {
        Self(None)
    }
}

impl SecretSource for StaticKey {
    fn api_key(&self) -> Option<ApiKey> {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_everywhere() {
        let key = ApiKey::new("sk-secret-123");
        assert!(!format!("{key:?} {key}").contains("secret"));
        assert_eq!(
            key.redact_in("zły klucz sk-secret-123!"),
            "zły klucz [REDACTED]!"
        );
        assert_eq!(ApiKey::new(" ").redact_in("x"), "x");
        assert!(StaticKey::none().api_key().is_none());
        let src = StaticKey::new("k");
        assert_eq!(
            src.api_key().map(|k| k.expose().to_owned()).as_deref(),
            Some("k")
        );
        assert_eq!(format!("{src:?}"), "StaticKey(Some(ApiKey([REDACTED])))");
    }
}
