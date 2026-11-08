//! Przypadki testu kontraktowego: błędy, limity czasu, anulowanie, prywatność, walidacja.

use std::time::Duration;

use futures_util::StreamExt;
use tokio::time::Instant;

use super::cases::{error_kind, provider, request, stop_reason};
use super::{CANCEL_BUDGET, FALLBACK_BUDGET, Harness, Scenario, collect};
use crate::CancellationToken;
use crate::error::ProviderErrorKind;
use crate::event::{ProviderEvent, StopReason};
use crate::privacy::{PrivacyTag, RequestPrivacy};
use crate::provider::ModelProvider;
use crate::request::ChatRequest;

type KindCheck = fn(&ProviderErrorKind) -> bool;

/// Błędy HTTP → sklasyfikowane `Error` w budżecie fallbacku (≤ 2 s).
pub async fn http_errors_are_classified<H: Harness>(h: &H) {
    let cases: [(u16, Option<u64>, KindCheck); 5] = [
        (400, None, |k| *k == ProviderErrorKind::InvalidRequest),
        (401, None, |k| *k == ProviderErrorKind::Auth),
        (429, Some(30), |k| {
            *k == ProviderErrorKind::RateLimited {
                retry_after_ms: Some(30_000),
            }
        }),
        (500, None, |k| {
            matches!(k, ProviderErrorKind::Server { status: 500 })
        }),
        (529, None, |k| {
            matches!(k, ProviderErrorKind::Overloaded { .. })
        }),
    ];
    for (status, retry_after_s, expected) in cases {
        let scenario = Scenario::HttpError {
            status,
            retry_after_s,
        };
        let Some(p) = provider(h, scenario).await else {
            continue;
        };
        let got = collect(p.stream(request(h), CancellationToken::new())).await;
        let kind = error_kind(got.terminal());
        assert!(
            kind.as_ref().is_some_and(expected),
            "HTTP {status} → {kind:?}"
        );
        assert!(
            got.terminal_at() <= FALLBACK_BUDGET,
            "HTTP {status}: {:?}",
            got.terminal_at()
        );
        assert!(h.wire_requests() >= 1);
    }
}

/// Odmowa to `Stop(Refusal)`, nie błąd (HTTP 200 u dostawcy).
pub async fn refusal_is_a_stop_not_an_error<H: Harness>(h: &H) {
    let Some(p) = provider(h, Scenario::Refusal).await else {
        return;
    };
    let got = collect(p.stream(request(h), CancellationToken::new())).await;
    assert_eq!(
        stop_reason(got.terminal()),
        Some(StopReason::Refusal),
        "{:?}",
        got.events
    );
}

/// Ucięcie na limicie → `Stop(MaxTokens)` z dotychczasowym tekstem.
pub async fn max_tokens_is_reported<H: Harness>(h: &H) {
    let text = "Bardzo długa odpowiedź, która".to_owned();
    let Some(p) = provider(h, Scenario::MaxTokens { text: text.clone() }).await else {
        return;
    };
    let got = collect(p.stream(request(h), CancellationToken::new())).await;
    assert_eq!(got.text(), text);
    assert_eq!(stop_reason(got.terminal()), Some(StopReason::MaxTokens));
}

/// Milczący dostawca → `Timeout` w budżecie fallbacku.
pub async fn stall_times_out_within_budget<H: Harness>(h: &H) {
    let Some(p) = provider(h, Scenario::Stall).await else {
        return;
    };
    let got = collect(p.stream(request(h), CancellationToken::new())).await;
    let kind = error_kind(got.terminal());
    assert!(
        matches!(kind, Some(ProviderErrorKind::Timeout { .. })),
        "{:?}",
        got.events
    );
    let limit = (h.stall_timeout() * 2).min(FALLBACK_BUDGET);
    assert!(
        got.terminal_at() <= limit,
        "timeout po {:?} > {limit:?}",
        got.terminal_at()
    );
}

/// Anulowanie w trakcie → `Stop(Cancelled)` ≤ 100 ms.
pub async fn cancel_ends_stream_fast<H: Harness>(h: &H) {
    let scenario = Scenario::Slow {
        chunks: (0..50).map(|i| format!("słowo{i} ")).collect(),
        interval: Duration::from_millis(40),
    };
    let Some(p) = provider(h, scenario).await else {
        return;
    };
    let cancel = CancellationToken::new();
    let mut stream = p.stream(request(h), cancel.clone());
    loop {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(ProviderEvent::TextDelta { .. })) => break,
            Ok(Some(ev)) if ev.is_terminal() => panic!("koniec przed treścią: {ev:?}"),
            Ok(Some(_)) => {}
            other => panic!("brak pierwszej treści: {other:?}"),
        }
    }
    let at = Instant::now();
    cancel.cancel();
    let mut terminal = None;
    while let Ok(Some(ev)) = tokio::time::timeout(Duration::from_secs(2), stream.next()).await {
        if ev.is_terminal() {
            terminal = Some((at.elapsed(), ev));
            break;
        }
    }
    let (took, ev) = terminal.unwrap_or_else(|| panic!("brak zdarzenia końcowego po anulowaniu"));
    assert_eq!(stop_reason(&ev), Some(StopReason::Cancelled));
    assert!(took <= CANCEL_BUDGET, "anulowanie trwało {took:?}");
    assert!(
        stream.next().await.is_none(),
        "po zdarzeniu końcowym strumień musi się skończyć"
    );
}

/// Token anulowany przed startem → `Stop(Cancelled)` bez ruchu sieciowego.
pub async fn pre_cancelled_sends_nothing<H: Harness>(h: &H) {
    let Some(p) = provider(
        h,
        Scenario::Text {
            chunks: vec!["x".into()],
        },
    )
    .await
    else {
        return;
    };
    let cancel = CancellationToken::new();
    cancel.cancel();
    let got = collect(p.stream(request(h), cancel)).await;
    assert_eq!(stop_reason(got.terminal()), Some(StopReason::Cancelled));
    assert_eq!(h.wire_requests(), 0);
}

/// Sesja prywatna do dostawcy CN/„może trenować" → `PrivacyBlocked`, zero żądań na drucie.
pub async fn private_request_is_blocked_before_wire<H: Harness>(h: &H) {
    let p = h.provider_blocking_private().await;
    let mut req = request(h);
    req.meta.privacy = RequestPrivacy {
        tag: PrivacyTag::Private,
        jurisdiction_allow: vec![],
    };
    let got = collect(p.stream(req, CancellationToken::new())).await;
    assert_eq!(
        error_kind(got.terminal()),
        Some(ProviderErrorKind::PrivacyBlocked)
    );
    assert_eq!(
        h.wire_requests(),
        0,
        "obrona w głąb: nic nie może wyjść do sieci"
    );
}

/// Nieprawidłowe żądanie → `InvalidRequest` lokalnie, bez fallbacku i bez sieci.
pub async fn invalid_request_is_rejected_locally<H: Harness>(h: &H) {
    let Some(p) = provider(
        h,
        Scenario::Text {
            chunks: vec!["x".into()],
        },
    )
    .await
    else {
        return;
    };
    let got = collect(p.stream(
        ChatRequest::new(h.model(), vec![]),
        CancellationToken::new(),
    ))
    .await;
    match got.terminal() {
        ProviderEvent::Error(e) => {
            assert_eq!(e.kind, ProviderErrorKind::InvalidRequest);
            assert!(!e.should_fallback());
        }
        other => panic!("oczekiwano błędu: {other:?}"),
    }
    assert_eq!(h.wire_requests(), 0);
}
