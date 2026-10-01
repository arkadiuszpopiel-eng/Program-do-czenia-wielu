//! Konfiguracja adapterów: endpoint, uwierzytelnienie, limity czasu, ponawianie, profil dostawcy.

use std::collections::BTreeMap;
use std::time::Duration;

use providers_contract::{
    ApiKey, Effort, ModelCapabilities, PricingTable, ProviderError, ProviderErrorKind, ProviderId,
    ProviderPrivacy,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

/// Sposób przekazania klucza API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthScheme {
    /// `Authorization: Bearer <klucz>` (OpenAI i zgodne).
    Bearer,
    /// `x-api-key: <klucz>` (Anthropic i zgodne).
    XApiKey,
    /// Własny nagłówek z samym kluczem.
    Header(String),
    /// Bez uwierzytelnienia (np. Ollama/LM Studio na localhost).
    None,
}

impl AuthScheme {
    /// Czy schemat wymaga klucza.
    pub fn requires_key(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Dopisuje nagłówek uwierzytelnienia (oznaczony jako wrażliwy — nie trafi do `Debug`).
    pub(crate) fn apply(
        &self,
        headers: &mut HeaderMap,
        key: Option<&ApiKey>,
    ) -> Result<(), ProviderError> {
        let Some(key) = key.filter(|k| !k.is_empty()) else {
            return if self.requires_key() {
                Err(ProviderError::new(
                    ProviderErrorKind::Auth,
                    "brak klucza API — dostawca nieskonfigurowany (dodaj klucz w Ustawieniach)",
                ))
            } else {
                Ok(())
            };
        };
        let (name, value) = match self {
            Self::Bearer => (
                HeaderName::from_static("authorization"),
                format!("Bearer {}", key.expose()),
            ),
            Self::XApiKey => (
                HeaderName::from_static("x-api-key"),
                key.expose().to_owned(),
            ),
            Self::Header(name) => (
                HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                    ProviderError::invalid_request(format!("zła nazwa nagłówka `{name}`"))
                })?,
                key.expose().to_owned(),
            ),
            Self::None => return Ok(()),
        };
        let mut value = HeaderValue::from_str(&value).map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Auth,
                "klucz API zawiera niedozwolone znaki",
            )
        })?;
        value.set_sensitive(true);
        headers.insert(name, value);
        Ok(())
    }
}

/// Limity czasu (SPEC: przełączenie fallback ≤ 2 s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// Nawiązanie połączenia TCP/TLS.
    pub connect: Duration,
    /// Od wysłania żądania do pierwszego zdarzenia strumienia (nagłówki + pierwsze zdarzenie SSE).
    pub first_token: Duration,
    /// Maksymalna przerwa między porcjami strumienia (dostawcy wysyłają `ping`).
    pub idle: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(5),
            first_token: Duration::from_secs(20),
            idle: Duration::from_secs(30),
        }
    }
}

/// Polityka ponawiania: tylko błędy idempotentne (odrzucenie przed przetworzeniem: 429/5xx/529,
/// błąd połączenia) i tylko przed pierwszym tokenem; wykładniczy backoff z jitterem; łączny
/// budżet czasu tak, by Router dostał błąd w ≤ 2 s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Maksymalna liczba ponowień (0 = bez ponawiania).
    pub max_retries: u32,
    /// Opóźnienie bazowe.
    pub base_delay: Duration,
    /// Górny limit pojedynczego opóźnienia.
    pub max_delay: Duration,
    /// Najdłuższy `retry-after`, na który adapter poczeka (dłuższy → błąd od razu, Router przełącza).
    pub max_retry_after: Duration,
    /// Łączny budżet czasu ponowień od startu żądania.
    pub budget: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::from_millis(150),
            max_delay: Duration::from_millis(800),
            max_retry_after: Duration::from_secs(1),
            budget: Duration::from_millis(1_500),
        }
    }
}

/// Konfiguracja HTTP endpointu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpConfig {
    /// Adres bazowy (Anthropic: bez `/v1`; OpenAI i zgodne: z `/v1`).
    pub base_url: String,
    /// Uwierzytelnienie.
    pub auth: AuthScheme,
    /// Limity czasu.
    pub timeouts: Timeouts,
    /// Ponawianie.
    pub retry: RetryPolicy,
    /// Dodatkowe nagłówki (np. atrybucja OpenRouter) — nigdy sekrety.
    pub extra_headers: Vec<(String, String)>,
}

impl HttpConfig {
    /// Konfiguracja z domyślnymi limitami.
    pub fn new(base_url: impl Into<String>, auth: AuthScheme) -> Self {
        Self {
            base_url: base_url.into(),
            auth,
            timeouts: Timeouts::default(),
            retry: RetryPolicy::default(),
            extra_headers: Vec::new(),
        }
    }

    /// Pełny URL dla ścieżki (`/v1/messages`, `/chat/completions`…).
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }
}

/// Profil dostawcy: tożsamość, prywatność, modele, cennik (z katalogu i konfiguracji użytkownika).
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderProfile {
    /// Identyfikator (wpis katalogu).
    pub id: ProviderId,
    /// Tag prywatności i jurysdykcja.
    pub privacy: ProviderPrivacy,
    /// Model domyślny.
    pub default_model: Option<String>,
    /// Możliwości modeli skonfigurowane jawnie (wygrywają z tabelą znanych modeli i Models API).
    pub models: BTreeMap<String, ModelCapabilities>,
    /// Tabela cen (konfiguracja; nigdy z kodu).
    pub pricing: PricingTable,
    /// `max_tokens`, gdy nie podaje go żądanie ani możliwości modelu.
    pub default_max_tokens: u32,
    /// Wysiłek wysyłany jawnie, gdy żądanie go nie podaje (SPEC: `medium`).
    pub default_effort: Effort,
}

impl ProviderProfile {
    /// Profil z domyślnymi wartościami (prywatność `unknown` — ostrożnie).
    pub fn new(id: impl Into<ProviderId>) -> Self {
        Self {
            id: id.into(),
            privacy: ProviderPrivacy::default(),
            default_model: None,
            models: BTreeMap::new(),
            pricing: PricingTable::new(),
            default_max_tokens: 8_192,
            default_effort: Effort::Medium,
        }
    }
}

/// Błąd konfiguracji adaptera.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Brak adresu endpointu (katalog go nie podaje, użytkownik też nie).
    #[error("dostawca `{0}`: brak base_url (uzupełnij w kreatorze)")]
    MissingBaseUrl(String),
    /// Błędny wpis katalogu.
    #[error("wpis katalogu: {0}")]
    Catalog(String),
    /// Trasa zabroniona w rejestrze zgodności.
    #[error("dostawca `{0}` ma status zgodności `forbidden`")]
    Forbidden(String),
    /// Dostawca nie jest czatem albo nie ma adaptera natywnego.
    #[error("dostawca `{0}`: brak adaptera czatu ({1})")]
    Unsupported(String, String),
    /// Nie udało się zbudować klienta HTTP.
    #[error("klient HTTP: {0}")]
    Http(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_headers_are_sensitive_and_required() {
        let key = ApiKey::new("sk-test");
        let mut h = HeaderMap::new();
        AuthScheme::Bearer.apply(&mut h, Some(&key)).unwrap();
        let v = h.get("authorization").unwrap();
        assert!(v.is_sensitive());
        assert!(!format!("{h:?}").contains("sk-test"));
        let mut h = HeaderMap::new();
        AuthScheme::Header("api-key".into())
            .apply(&mut h, Some(&key))
            .unwrap();
        assert!(h.contains_key("api-key"));
        let err = AuthScheme::XApiKey
            .apply(&mut HeaderMap::new(), None)
            .unwrap_err();
        assert_eq!(err.kind, ProviderErrorKind::Auth);
        assert!(AuthScheme::None.apply(&mut HeaderMap::new(), None).is_ok());
        assert!(
            AuthScheme::Header("zły nagłówek".into())
                .apply(&mut HeaderMap::new(), Some(&key))
                .is_err()
        );
    }

    #[test]
    fn url_join() {
        let c = HttpConfig::new("https://api.x.ai/v1/", AuthScheme::Bearer);
        assert_eq!(
            c.url("/chat/completions"),
            "https://api.x.ai/v1/chat/completions"
        );
    }
}
