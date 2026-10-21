//! Wynik generacji (do zapisu w historii i zdarzeń końcowych) i klasyfikacja zakończenia strumienia
//! dostawcy: status tury, powód zatrzymania, błąd z komunikatem PL bez sekretów.

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use app_api::dto::{self, StopReason, TurnError, TurnErrorCode, TurnStatus};
use futures_util::FutureExt;
use providers_contract::{ProviderError, ProviderErrorKind, StopReason as PStop, Usage};
use sessions_contract::Block as SBlock;

/// Co dostawca wybrał.
#[derive(Debug, Clone, Default)]
pub(crate) struct Chosen {
    pub provider_id: String,
    pub provider_name: String,
    pub account: Option<String>,
    pub model: String,
}

/// Wynik generacji (do zapisu w historii i zdarzeń końcowych).
#[derive(Debug, Clone)]
pub(crate) struct Outcome {
    pub text: String,
    pub thinking: Vec<SBlock>,
    pub status: TurnStatus,
    pub stop: Option<StopReason>,
    pub error: Option<TurnError>,
    pub usage: Option<Usage>,
    pub cost_nano_usd: Option<u64>,
    pub chosen: Option<Chosen>,
    pub latency_ms: u64,
    pub thinking_ms: Option<u64>,
    /// Kroki narzędzi (przebieg agentki).
    pub tools: Vec<dto::ToolStep>,
    /// Karta „czeka na zatwierdzenie" (przebieg agentki).
    pub approval: Option<dto::ApprovalPending>,
    /// Odpowiedź polecenia bez modelu (nazwa polecenia na osi czasu).
    pub local: Option<&'static str>,
}

impl Outcome {
    pub(crate) fn failed(error: TurnError) -> Self {
        Self {
            text: String::new(),
            thinking: Vec::new(),
            status: TurnStatus::Error,
            stop: None,
            error: Some(error),
            usage: None,
            cost_nano_usd: None,
            chosen: None,
            latency_ms: 0,
            thinking_ms: None,
            tools: Vec::new(),
            approval: None,
            local: None,
        }
    }
}

pub(crate) fn turn_error(
    code: TurnErrorCode,
    message: impl Into<String>,
    provider: Option<&str>,
) -> TurnError {
    TurnError {
        code,
        message: message.into(),
        retry_at: None,
        provider: provider.map(str::to_owned),
    }
}

/// Generacja ze strażnikiem: panika w przebiegu kończy turę błędem zamiast zostawić ją na zawsze
/// w stanie „pisze" z zajętą sesją. Działa w buildach z odwijaniem stosu; build wydaniowy powłoki
/// ma `panic = "abort"` — tam proces kończy się po zapisie paniki do dziennika (`app-logs`).
pub(crate) async fn guarded(run: impl Future<Output = Outcome>) -> Outcome {
    match AssertUnwindSafe(run).catch_unwind().await {
        Ok(outcome) => outcome,
        Err(_) => {
            tracing::error!("generacja przerwana paniką — tura zakończona błędem");
            Outcome::failed(turn_error(
                TurnErrorCode::Provider,
                "Odpowiedź przerwana przez błąd wewnętrzny Alfy. Szczegóły są w dzienniku; spróbuj ponownie.",
                None,
            ))
        }
    }
}

/// Identyfikator dostawcy modeli lokalnych (`providers-local`).
const LOCAL_PROVIDER: &str = "local";

/// Komunikat jako zdanie: wielka litera na początku i kropka na końcu.
fn sentence(text: &str) -> String {
    let mut chars = text.trim().chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out: String = first.to_uppercase().chain(chars).collect();
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// Brak środków na koncie (402, `insufficient_quota`, „credit balance is too low”) — dostawcy
/// zgłaszają go jako błąd klucza albo żądania, a to nie jest odrzucony klucz (przegląd 2026-10-21).
fn no_credit(e: &ProviderError) -> bool {
    let code = e
        .provider_code
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    let message = e.message.to_lowercase();
    e.status == Some(402)
        || ["insufficient_quota", "billing", "credit"]
            .iter()
            .any(|c| code.contains(c))
        || [
            "credit balance",
            "insufficient balance",
            "insufficient_quota",
        ]
        .iter()
        .any(|m| message.contains(m))
}

/// Błąd dostawcy → błąd tury (komunikat PL bez sekretów).
pub(crate) fn provider_error(e: &ProviderError, provider: &str) -> TurnError {
    let now = chrono::Utc::now();
    match &e.kind {
        // Brak środków przed limitem zapytań: 429 `billing_not_active` (OpenAI) to nie chwilowy limit.
        ProviderErrorKind::Auth
        | ProviderErrorKind::InvalidRequest
        | ProviderErrorKind::RateLimited { .. }
            if no_credit(e) =>
        {
            turn_error(
                TurnErrorCode::Provider,
                format!(
                    "Brak środków na koncie u dostawcy {provider} — doładuj konto albo wybierz inne w Ustawieniach."
                ),
                Some(provider),
            )
        }
        ProviderErrorKind::RateLimited { retry_after_ms } => {
            let wait = retry_after_ms.unwrap_or(60_000);
            let at = now + chrono::Duration::milliseconds(i64::try_from(wait).unwrap_or(60_000));
            TurnError {
                code: TurnErrorCode::RateLimited,
                message: format!("Limit zapytań u dostawcy {provider}. Spróbuj ponownie później."),
                retry_at: Some(dto::iso(at)),
                provider: Some(provider.to_owned()),
            }
        }
        ProviderErrorKind::Network
        | ProviderErrorKind::Timeout {
            phase: providers_contract::TimeoutPhase::Connect,
        } => turn_error(
            TurnErrorCode::Offline,
            format!("Brak połączenia z dostawcą {provider}. Wiadomość możesz ponowić."),
            Some(provider),
        ),
        ProviderErrorKind::Auth => turn_error(
            TurnErrorCode::Provider,
            format!("Dostawca {provider} odrzucił klucz API — sprawdź konto w Ustawieniach."),
            Some(provider),
        ),
        // Model lokalny to nie „dostawca": jego komunikaty (brak modelu, brak silnika, za mało
        // pamięci) są już pełnymi zdaniami z podpowiedzią — bez przedrostka.
        _ if provider == LOCAL_PROVIDER => turn_error(
            TurnErrorCode::Provider,
            sentence(&e.message),
            Some(provider),
        ),
        _ => turn_error(
            TurnErrorCode::Provider,
            format!("Błąd dostawcy {provider}: {e}"),
            Some(provider),
        ),
    }
}

pub(crate) fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn classify(
    cancelled: bool,
    turn: &providers_contract::AssistantTurn,
    provider: &str,
) -> (TurnStatus, Option<StopReason>, Option<TurnError>) {
    if cancelled || turn.stop == Some(PStop::Cancelled) {
        return (TurnStatus::Cancelled, Some(StopReason::Cancelled), None);
    }
    if let Some(e) = &turn.error {
        return (TurnStatus::Error, None, Some(provider_error(e, provider)));
    }
    match turn.stop {
        Some(PStop::EndTurn | PStop::StopSequence | PStop::PauseTurn) => {
            (TurnStatus::Complete, Some(StopReason::End), None)
        }
        Some(PStop::MaxTokens) => (TurnStatus::Complete, Some(StopReason::MaxTokens), None),
        Some(PStop::ToolUse) => (TurnStatus::Complete, Some(StopReason::ToolUse), None),
        Some(PStop::Refusal) => (TurnStatus::Complete, Some(StopReason::Refusal), None),
        Some(PStop::ContextWindowExceeded) => (
            TurnStatus::Error,
            None,
            Some(turn_error(
                TurnErrorCode::ContextOverflow,
                "Rozmowa przekroczyła okno kontekstu modelu — zacznij nową gałąź lub sesję.",
                Some(provider),
            )),
        ),
        Some(PStop::Cancelled) => (TurnStatus::Cancelled, Some(StopReason::Cancelled), None),
        None => (
            TurnStatus::Error,
            None,
            Some(turn_error(
                TurnErrorCode::Provider,
                format!("Strumień dostawcy {provider} urwał się bez zakończenia."),
                Some(provider),
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-09: panika w przebiegu daje turę z błędem, a zwykły wynik przechodzi bez zmian.
    #[tokio::test]
    async fn panicking_generation_ends_as_failed_turn() {
        let failed = guarded(async {
            if std::hint::black_box(true) {
                std::panic::panic_any("awaria testowa");
            }
            Outcome::failed(turn_error(TurnErrorCode::Offline, "inny", None))
        })
        .await;
        assert_eq!(failed.status, TurnStatus::Error);
        let error = failed.error.expect("błąd tury");
        assert_eq!(error.code, TurnErrorCode::Provider);
        assert!(
            error.message.contains("błąd wewnętrzny"),
            "{}",
            error.message
        );
        let kept =
            guarded(async { Outcome::failed(turn_error(TurnErrorCode::Offline, "sieć", None)) })
                .await;
        assert_eq!(kept.error.map(|e| e.code), Some(TurnErrorCode::Offline));
    }

    #[test]
    fn local_model_errors_are_plain_sentences_without_provider_prefix() {
        let e = ProviderError::new(
            ProviderErrorKind::Unsupported,
            "brak silnika modeli lokalnych (llama-server, wersja cuda) — pobierz go w Ustawieniach → Modele lokalne i silniki",
        );
        let turn = provider_error(&e, "local");
        assert_eq!(turn.code, TurnErrorCode::Provider);
        assert_eq!(turn.provider.as_deref(), Some("local"));
        assert_eq!(
            turn.message,
            "Brak silnika modeli lokalnych (llama-server, wersja cuda) — pobierz go w Ustawieniach → Modele lokalne i silniki."
        );
    }

    #[test]
    fn cloud_provider_errors_keep_the_provider_name() {
        let e = ProviderError::new(ProviderErrorKind::Unsupported, "nieobsługiwane");
        let turn = provider_error(&e, "anthropic");
        assert!(
            turn.message.starts_with("Błąd dostawcy anthropic:"),
            "{}",
            turn.message
        );
    }

    #[test]
    fn missing_credit_is_not_reported_as_a_rejected_key() {
        let cases = [
            ProviderError::new(ProviderErrorKind::Auth, "Insufficient Balance").with_status(402),
            ProviderError::new(ProviderErrorKind::Auth, "quota")
                .with_provider_code("insufficient_quota"),
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "Your credit balance is too low to access the API",
            )
            .with_status(400),
        ];
        let billing = ProviderError::new(
            ProviderErrorKind::RateLimited {
                retry_after_ms: None,
            },
            "billing",
        )
        .with_provider_code("Billing_Not_Active");
        for e in cases.into_iter().chain([billing]) {
            let turn = provider_error(&e, "deepseek");
            assert!(
                turn.message
                    .starts_with("Brak środków na koncie u dostawcy deepseek"),
                "{}",
                turn.message
            );
        }
        let key = ProviderError::new(ProviderErrorKind::Auth, "invalid x-api-key").with_status(401);
        assert!(
            provider_error(&key, "anthropic")
                .message
                .contains("odrzucił klucz")
        );
    }

    #[test]
    fn sentence_capitalises_and_terminates() {
        assert_eq!(sentence("  już zdanie.  "), "Już zdanie.");
        assert_eq!(sentence("łódź"), "Łódź.");
        assert_eq!(sentence(""), "");
    }
}
