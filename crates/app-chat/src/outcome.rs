//! Wynik generacji (do zapisu w historii i zdarzeń końcowych) i klasyfikacja zakończenia strumienia
//! dostawcy: status tury, powód zatrzymania, błąd z komunikatem PL bez sekretów.

use std::time::Duration;

use app_api::dto::{self, StopReason, TurnError, TurnErrorCode, TurnStatus};
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

/// Błąd dostawcy → błąd tury (komunikat PL bez sekretów).
pub(crate) fn provider_error(e: &ProviderError, provider: &str) -> TurnError {
    let now = chrono::Utc::now();
    match &e.kind {
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
