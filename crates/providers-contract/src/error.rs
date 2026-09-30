//! Błędy dostawców sklasyfikowane tak, by Router mógł zdecydować o fallbacku w ≤ 2 s.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Faza, w której upłynął limit czasu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutPhase {
    /// Nawiązanie połączenia (TCP/TLS).
    Connect,
    /// Od wysłania żądania do pierwszego zdarzenia strumienia.
    FirstToken,
    /// Przerwa między zdarzeniami strumienia.
    Idle,
}

/// Rodzaj błędu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderErrorKind {
    /// 429 — limit zapytań/tokenów. `retry_after_ms` z nagłówka `retry-after`, jeśli był.
    RateLimited {
        /// Sugerowane opóźnienie ponowienia.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retry_after_ms: Option<u64>,
    },
    /// 529/503 — przeciążenie dostawcy.
    Overloaded {
        /// Sugerowane opóźnienie ponowienia.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retry_after_ms: Option<u64>,
    },
    /// 401/402/403 — brak lub zły klucz, brak uprawnień, rozliczenia, dostawca nieskonfigurowany.
    Auth,
    /// 400/404/413/422 — żądanie odrzucone (nie ponawiać tego samego ciała).
    InvalidRequest,
    /// Błąd sieci (DNS, reset połączenia, urwany strumień).
    Network,
    /// Przekroczony limit czasu.
    Timeout {
        /// Faza.
        phase: TimeoutPhase,
    },
    /// 5xx po stronie dostawcy.
    Server {
        /// Kod HTTP.
        status: u16,
    },
    /// Żądanie „prywatne" do dostawcy z tagiem CN/„może trenować" — odmowa adaptera (obrona w głąb).
    PrivacyBlocked,
    /// Funkcja nieobsługiwana przez dostawcę/model (np. embeddings u Anthropic).
    Unsupported,
    /// Dostawca przysłał niezrozumiały strumień/JSON.
    Protocol,
}

impl ProviderErrorKind {
    /// Czy adapter może ponowić **to samo** żądanie przed pierwszym tokenem (idempotentne odrzucenie).
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. }
                | Self::Overloaded { .. }
                | Self::Server { .. }
                | Self::Network
                | Self::Timeout {
                    phase: TimeoutPhase::Connect
                }
        )
    }

    /// Czy Router powinien przełączyć się na inny cel (ta sama historia, bez utraty wiadomości).
    pub fn should_fallback(&self) -> bool {
        !matches!(self, Self::InvalidRequest | Self::PrivacyBlocked)
    }

    /// Sugerowane opóźnienie z nagłówka dostawcy.
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            Self::RateLimited { retry_after_ms } | Self::Overloaded { retry_after_ms } => {
                *retry_after_ms
            }
            _ => None,
        }
    }
}

impl fmt::Display for ProviderErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RateLimited { .. } => f.write_str("limit zapytań (429)"),
            Self::Overloaded { .. } => f.write_str("dostawca przeciążony"),
            Self::Auth => f.write_str("błąd uwierzytelnienia lub uprawnień"),
            Self::InvalidRequest => f.write_str("nieprawidłowe żądanie"),
            Self::Network => f.write_str("błąd sieci"),
            Self::Timeout { phase } => write!(f, "przekroczony czas ({phase:?})"),
            Self::Server { status } => write!(f, "błąd serwera dostawcy ({status})"),
            Self::PrivacyBlocked => f.write_str("zablokowane przez politykę prywatności"),
            Self::Unsupported => f.write_str("nieobsługiwane przez dostawcę"),
            Self::Protocol => f.write_str("błąd protokołu dostawcy"),
        }
    }
}

/// Błąd dostawcy. Komunikat nigdy nie zawiera klucza API (adapter redaguje treść odpowiedzi).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error)]
#[error("{kind}: {message}")]
pub struct ProviderError {
    /// Klasyfikacja.
    pub kind: ProviderErrorKind,
    /// Opis (po polsku lub komunikat dostawcy, skrócony i zredagowany).
    pub message: String,
    /// Kod HTTP, jeśli był.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// Typ błędu dostawcy (np. `overloaded_error`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
    /// Identyfikator żądania u dostawcy (do zgłoszeń).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Czy przed błędem wyemitowano już treść (tekst/myślenie/narzędzie). Router przy fallbacku
    /// odrzuca wtedy częściową odpowiedź i powtarza całe żądanie na kolejnym celu.
    #[serde(default)]
    pub after_output: bool,
}

impl ProviderError {
    /// Nowy błąd danego rodzaju.
    pub fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            status: None,
            provider_code: None,
            request_id: None,
            after_output: false,
        }
    }

    /// Skrót: `InvalidRequest`.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(ProviderErrorKind::InvalidRequest, message)
    }

    /// Ustawia kod HTTP.
    pub fn with_status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }

    /// Ustawia typ błędu dostawcy.
    pub fn with_provider_code(mut self, code: impl Into<String>) -> Self {
        self.provider_code = Some(code.into());
        self
    }

    /// Ustawia identyfikator żądania.
    pub fn with_request_id(mut self, id: Option<String>) -> Self {
        self.request_id = id;
        self
    }

    /// Oznacza, że błąd wystąpił po wyemitowaniu treści.
    pub fn after_output(mut self, after: bool) -> Self {
        self.after_output = after;
        self
    }

    /// Czy adapter może ponowić żądanie (tylko przed pierwszym tokenem).
    pub fn is_retryable(&self) -> bool {
        !self.after_output && self.kind.is_retryable()
    }

    /// Czy Router powinien przełączyć się na inny cel.
    pub fn should_fallback(&self) -> bool {
        self.kind.should_fallback()
    }
}

/// Wspólna klasyfikacja kodu HTTP (adaptery mogą ją doprecyzować po treści odpowiedzi).
///
/// ```
/// use providers_contract::{classify_http_status, ProviderErrorKind};
/// assert_eq!(
///     classify_http_status(429, Some(1500)),
///     ProviderErrorKind::RateLimited { retry_after_ms: Some(1500) }
/// );
/// assert_eq!(classify_http_status(529, None), ProviderErrorKind::Overloaded { retry_after_ms: None });
/// assert!(!classify_http_status(400, None).should_fallback());
/// ```
pub fn classify_http_status(status: u16, retry_after_ms: Option<u64>) -> ProviderErrorKind {
    match status {
        401..=403 => ProviderErrorKind::Auth,
        408 => ProviderErrorKind::Server { status },
        429 => ProviderErrorKind::RateLimited { retry_after_ms },
        503 | 529 => ProviderErrorKind::Overloaded { retry_after_ms },
        500..=599 => ProviderErrorKind::Server { status },
        _ => ProviderErrorKind::InvalidRequest,
    }
}

/// Parsuje nagłówek `retry-after` (sekundy, także ułamkowe) na milisekundy.
/// Format daty HTTP jest pomijany (`None`) — adapter użyje wtedy własnego backoffu.
pub fn parse_retry_after_ms(value: &str) -> Option<u64> {
    let secs: f64 = value.trim().parse().ok()?;
    if !secs.is_finite() || secs < 0.0 {
        return None;
    }
    // Wartości nagłówka są małe (sekundy–minuty); obcięcie do u64 ms jest bezpieczne.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let ms = (secs * 1000.0).round() as u64;
    Some(ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_table() {
        assert_eq!(classify_http_status(401, None), ProviderErrorKind::Auth);
        assert_eq!(classify_http_status(402, None), ProviderErrorKind::Auth);
        assert_eq!(
            classify_http_status(404, None),
            ProviderErrorKind::InvalidRequest
        );
        assert_eq!(
            classify_http_status(413, None),
            ProviderErrorKind::InvalidRequest
        );
        assert_eq!(
            classify_http_status(500, None),
            ProviderErrorKind::Server { status: 500 }
        );
        assert!(classify_http_status(500, None).is_retryable());
        assert!(classify_http_status(503, Some(5)).is_retryable());
        assert!(!classify_http_status(401, None).is_retryable());
        assert!(classify_http_status(401, None).should_fallback());
    }

    #[test]
    fn after_output_blocks_retry() {
        let e = ProviderError::new(ProviderErrorKind::Network, "reset").after_output(true);
        assert!(!e.is_retryable());
        assert!(e.should_fallback());
        let t = ProviderErrorKind::Timeout {
            phase: TimeoutPhase::FirstToken,
        };
        assert!(
            !t.is_retryable(),
            "timeout po wysłaniu nie jest idempotentny"
        );
    }

    #[test]
    fn retry_after_parsing() {
        assert_eq!(parse_retry_after_ms("2"), Some(2000));
        assert_eq!(parse_retry_after_ms("0.25"), Some(250));
        assert_eq!(parse_retry_after_ms("Wed, 21 Oct 2015 07:28:00 GMT"), None);
        assert_eq!(parse_retry_after_ms("-1"), None);
    }

    #[test]
    fn display_is_polish_and_serde_tagged() {
        let e = ProviderError::new(
            ProviderErrorKind::Overloaded {
                retry_after_ms: None,
            },
            "x",
        );
        assert_eq!(e.to_string(), "dostawca przeciążony: x");
        let json = serde_json::to_value(&e).ok();
        assert_eq!(
            json.and_then(|v| v["kind"]["kind"].as_str().map(str::to_owned))
                .as_deref(),
            Some("overloaded")
        );
    }
}
