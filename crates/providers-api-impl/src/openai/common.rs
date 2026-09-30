//! Elementy wspólne formatów OpenAI (Chat Completions i Responses): błędy, obrazy, zużycie, wysiłek.

use providers_contract::{
    Effort, ImageSource, ProviderError, ProviderErrorKind, StopReason, ToolResultPart, Usage,
    classify_http_status, parse_retry_after_ms,
};
use reqwest::header::HeaderMap;
use serde_json::Value;

/// URL obrazu: `data:` dla base64, bez zmian dla URL; referencje muszą być rozwiązane wcześniej.
pub(crate) fn image_url(source: &ImageSource) -> Result<String, ProviderError> {
    match source {
        ImageSource::Base64 { media_type, data } => Ok(format!("data:{media_type};base64,{data}")),
        ImageSource::Url { url } => Ok(url.clone()),
        ImageSource::Ref { id, .. } => Err(ProviderError::invalid_request(format!(
            "nierozwiązana referencja obrazu `{id}` — rozwiąż ją przed wywołaniem"
        ))),
    }
}

/// Tekst wyniku narzędzia (obrazy w wynikach nie są przenoszone przez format funkcji).
pub(crate) fn tool_result_text(parts: &[ToolResultPart], is_error: bool) -> String {
    let text: Vec<&str> = parts
        .iter()
        .map(|p| match p {
            ToolResultPart::Text { text } => text.as_str(),
            ToolResultPart::Image { .. } => {
                "[obraz pominięty — format funkcji OpenAI nie przenosi obrazów]"
            }
        })
        .collect();
    let joined = text.join("\n");
    if is_error {
        format!("BŁĄD: {joined}")
    } else {
        joined
    }
}

/// Wysiłek na drucie (`reasoning_effort` / `reasoning.effort`): poziomy powyżej `high` → `high`.
pub(crate) fn effort(e: Effort) -> &'static str {
    match e {
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High | Effort::XHigh | Effort::Max => "high",
    }
}

/// Normalizacja zużycia: `input_tokens` bez cache (OpenAI wlicza cache do `prompt_tokens`).
pub(crate) fn usage(prompt: u64, cached: u64, completion: u64) -> Usage {
    Usage {
        input_tokens: prompt.saturating_sub(cached),
        output_tokens: completion,
        cache_read_tokens: cached,
        cache_write_tokens: 0,
    }
}

/// Mapowanie `finish_reason` Chat Completions.
pub(crate) fn finish_reason(reason: &str) -> StopReason {
    match reason {
        "length" => StopReason::MaxTokens,
        "tool_calls" | "function_call" => StopReason::ToolUse,
        "content_filter" => StopReason::Refusal,
        _ => StopReason::EndTurn,
    }
}

fn retry_after(headers: &HeaderMap) -> Option<u64> {
    let get = |name: &str| headers.get(name).and_then(|h| h.to_str().ok());
    get("retry-after-ms")
        .and_then(|v| v.trim().parse::<u64>().ok())
        .or_else(|| get("retry-after").and_then(parse_retry_after_ms))
}

/// Klasyfikacja błędu z ciała `{"error": {"message", "type", "code"}}`.
pub(crate) fn error_from_value(
    status: Option<u16>,
    retry_after_ms: Option<u64>,
    err: &Value,
) -> ProviderError {
    let code = err["code"]
        .as_str()
        .or_else(|| err["type"].as_str())
        .unwrap_or_default()
        .to_owned();
    // Kwota/klucz wygrywają (429 `insufficient_quota` nie jest chwilowy); potem kod HTTP;
    // bez kodu HTTP (błąd w strumieniu) — typ z ciała.
    let kind = match (code.as_str(), status) {
        ("insufficient_quota" | "invalid_api_key" | "billing_hard_limit_reached", _) => {
            ProviderErrorKind::Auth
        }
        (_, Some(s)) => classify_http_status(s, retry_after_ms),
        ("rate_limit_exceeded" | "rate_limit_error", None) => {
            ProviderErrorKind::RateLimited { retry_after_ms }
        }
        ("overloaded_error" | "server_overloaded", None) => {
            ProviderErrorKind::Overloaded { retry_after_ms }
        }
        ("invalid_request_error" | "invalid_prompt", None) => ProviderErrorKind::InvalidRequest,
        (_, None) => ProviderErrorKind::Server { status: 500 },
    };
    let message = err["message"]
        .as_str()
        .map_or_else(|| "błąd dostawcy".to_owned(), str::to_owned);
    let mut e = ProviderError::new(kind, message);
    if !code.is_empty() {
        e = e.with_provider_code(code);
    }
    if let Some(s) = status {
        e = e.with_status(s);
    }
    e
}

/// Klasyfikacja odpowiedzi HTTP z błędem.
pub(crate) fn classify(status: u16, headers: &HeaderMap, body: &str) -> ProviderError {
    let ra = retry_after(headers);
    let request_id = headers
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    let err = match serde_json::from_str::<Value>(body) {
        Ok(v) if v["error"].is_object() => error_from_value(Some(status), ra, &v["error"]),
        _ => ProviderError::new(classify_http_status(status, ra), format!("HTTP {status}"))
            .with_status(status),
    };
    err.with_request_id(request_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn quota_is_auth_and_retry_after_ms_wins() {
        let mut h = HeaderMap::new();
        h.insert("retry-after-ms", HeaderValue::from_static("250"));
        h.insert("retry-after", HeaderValue::from_static("9"));
        h.insert("x-request-id", HeaderValue::from_static("req_1"));
        let body = r#"{"error":{"message":"quota","type":"insufficient_quota","code":"insufficient_quota"}}"#;
        let e = classify(429, &h, body);
        assert_eq!(e.kind, ProviderErrorKind::Auth);
        assert_eq!(e.request_id.as_deref(), Some("req_1"));
        let rl = classify(
            429,
            &h,
            r#"{"error":{"message":"x","type":"requests","code":"rate_limit_exceeded"}}"#,
        );
        assert_eq!(
            rl.kind,
            ProviderErrorKind::RateLimited {
                retry_after_ms: Some(250)
            }
        );
        assert_eq!(
            classify(502, &HeaderMap::new(), "<html>").kind,
            ProviderErrorKind::Server { status: 502 }
        );
        assert_eq!(
            error_from_value(None, None, &serde_json::json!({})).kind,
            ProviderErrorKind::Server { status: 500 }
        );
    }

    #[test]
    fn helpers() {
        assert_eq!(
            usage(120, 100, 5),
            Usage {
                input_tokens: 20,
                output_tokens: 5,
                cache_read_tokens: 100,
                cache_write_tokens: 0
            }
        );
        assert_eq!(effort(Effort::Max), "high");
        assert_eq!(finish_reason("content_filter"), StopReason::Refusal);
        assert_eq!(
            tool_result_text(&[ToolResultPart::Text { text: "x".into() }], true),
            "BŁĄD: x"
        );
        let img = ImageSource::Base64 {
            media_type: "image/png".into(),
            data: "AAA".into(),
        };
        assert_eq!(image_url(&img).unwrap(), "data:image/png;base64,AAA");
        assert!(
            image_url(&ImageSource::Ref {
                id: "a".into(),
                media_type: "x".into()
            })
            .is_err()
        );
    }
}
