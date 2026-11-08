//! Przypadki testu kontraktowego: treść (tekst, narzędzia, myślenie, przerwane tury, koszt).
//! Każdy przypadek dostaje świeżego dostawcę z uchwytu.

use super::{Harness, Scenario, collect};
use crate::CancellationToken;
use crate::accumulate::TurnAccumulator;
use crate::error::ProviderErrorKind;
use crate::event::{ProviderEvent, StopReason, ToolArguments, Usage};
use crate::interruption::{InterruptionRendering, interruption_note};
use crate::message::{ContentBlock, Message};
use crate::provider::ModelProvider;
use crate::request::ChatRequest;

pub(super) fn request<H: Harness>(h: &H) -> ChatRequest {
    ChatRequest::new(h.model(), vec![Message::user_text("Cześć, jak się masz?")])
}

pub(super) async fn provider<H: Harness>(h: &H, scenario: Scenario) -> Option<H::Provider> {
    h.provider(scenario).await
}

pub(super) fn stop_reason(ev: &ProviderEvent) -> Option<StopReason> {
    match ev {
        ProviderEvent::Stop { reason, .. } => Some(*reason),
        _ => None,
    }
}

pub(super) fn error_kind(ev: &ProviderEvent) -> Option<ProviderErrorKind> {
    match ev {
        ProviderEvent::Error(e) => Some(e.kind.clone()),
        _ => None,
    }
}

/// Tekst: gramatyka, treść w kolejności, `Usage` przed końcem, `Stop(EndTurn)`, złożona tura.
pub async fn text_stream_follows_grammar<H: Harness>(h: &H) {
    let chunks = vec![
        "Dzień ".to_owned(),
        "dobry, ".into(),
        "zażółć gęślą jaźń!".into(),
    ];
    let Some(p) = provider(
        h,
        Scenario::Text {
            chunks: chunks.clone(),
        },
    )
    .await
    else {
        return;
    };
    let got = collect(p.stream(request(h), CancellationToken::new())).await;
    assert_eq!(got.text(), chunks.concat());
    assert_eq!(
        stop_reason(got.terminal()),
        Some(StopReason::EndTurn),
        "{:?}",
        got.events
    );
    let usage_pos = got
        .plain()
        .position(|e| matches!(e, ProviderEvent::Usage(_)));
    assert!(usage_pos.is_some(), "brak `Usage`: {:?}", got.events);
    let mut acc = TurnAccumulator::new(p.id().clone());
    got.plain().for_each(|e| acc.push(e));
    let turn = acc.finish();
    assert!(turn.is_complete());
    assert_eq!(turn.message.visible_text(), chunks.concat());
    assert!(
        turn.usage.output_tokens > 0,
        "output_tokens z `Usage`: {:?}",
        turn.usage
    );
    assert_eq!(h.wire_requests(), 1);
}

/// Narzędzie: start/koniec z identyfikatorem, poprawne argumenty, `Stop(ToolUse)`.
pub async fn tool_call_is_assembled<H: Harness>(h: &H) {
    let args = serde_json::json!({"city": "Kraków", "days": 2});
    let scenario = Scenario::ToolCall {
        id: "call_1".into(),
        name: "weather".into(),
        arguments: args.clone(),
    };
    let Some(p) = provider(h, scenario).await else {
        return;
    };
    let got = collect(p.stream(request(h), CancellationToken::new())).await;
    assert!(
        got.plain().any(
            |e| matches!(e, ProviderEvent::ToolCallStart { id, name, .. }
        if id == "call_1" && name == "weather")
        ),
        "{:?}",
        got.events
    );
    let end = got.plain().find_map(|e| match e {
        ProviderEvent::ToolCallEnd { arguments, .. } => Some(arguments.clone()),
        _ => None,
    });
    assert_eq!(
        end,
        Some(ToolArguments::Parsed {
            value: args.clone()
        })
    );
    assert_eq!(stop_reason(got.terminal()), Some(StopReason::ToolUse));
    let mut acc = TurnAccumulator::new(p.id().clone());
    got.plain().for_each(|e| acc.push(e));
    let turn = acc.finish();
    assert_eq!(
        turn.message.tool_uses().next().map(|t| &t.input),
        Some(&args)
    );
}

/// Myślenie: podpis i pochodzenie zachowane; w kolejnym żądaniu podpis wraca bez zmian.
pub async fn thinking_is_signed_and_replayed<H: Harness>(h: &H) {
    let scenario = Scenario::Thinking {
        thinking: "Rozważam odpowiedź.".into(),
        signature: "sig-EqQ0Zm9vYmFy".into(),
        text: "Odpowiedź.".into(),
    };
    let Some(p) = provider(h, scenario).await else {
        return;
    };
    let got = collect(p.stream(request(h), CancellationToken::new())).await;
    let mut acc = TurnAccumulator::new(p.id().clone());
    got.plain().for_each(|e| acc.push(e));
    let turn = acc.finish();
    let thinking = turn.message.content.iter().find_map(|b| match b {
        ContentBlock::Thinking(t) => Some(t.clone()),
        _ => None,
    });
    let thinking = thinking.unwrap_or_else(|| panic!("brak bloku myślenia: {:?}", got.events));
    // Podpis jest nieprzezroczysty: musi zawierać dane dostawcy (Responses API: `<id>:<encrypted>`).
    assert!(
        thinking
            .signature
            .as_deref()
            .is_some_and(|s| s.contains("sig-EqQ0Zm9vYmFy")),
        "{thinking:?}"
    );
    assert_eq!(thinking.text, "Rozważam odpowiedź.");
    assert_eq!(&thinking.provider_origin.provider, p.id());
    let mut next = request(h);
    next.messages.push(turn.message.clone());
    next.messages.push(Message::user_text("A dalej?"));
    let _ = collect(p.stream(next, CancellationToken::new())).await;
    let wire = h.last_wire_request().unwrap_or_default();
    assert!(
        wire.contains("sig-EqQ0Zm9vYmFy"),
        "podpis nie wrócił do dostawcy: {wire}"
    );
}

/// ACC-F1-providers-api-03: przerwana tura idzie w pełnej postaci, a notka po niej.
pub async fn interrupted_turn_is_rendered_append_only<H: Harness>(h: &H) {
    let Some(p) = provider(
        h,
        Scenario::Text {
            chunks: vec!["Dobrze.".into()],
        },
    )
    .await
    else {
        return;
    };
    let full = "Jutro w Krakowie będzie słonecznie, a wieczorem przelotny deszcz.";
    let heard = "Jutro w Krakowie będzie";
    let mut req = request(h);
    req.messages
        .push(Message::assistant_text(full).with_interruption(heard, false));
    req.messages.push(Message::user_text("Stop, a pojutrze?"));
    let got = collect(p.stream(req, CancellationToken::new())).await;
    assert_eq!(stop_reason(got.terminal()), Some(StopReason::EndTurn));
    let wire = h.last_wire_request().unwrap_or_default();
    let full_at = wire.find(full);
    assert!(
        full_at.is_some(),
        "pełna tura musi trafić do dostawcy bez zmian: {wire}"
    );
    if p.capabilities().interruption == InterruptionRendering::AppendNote {
        let note_at = wire.find(&interruption_note(heard, false));
        assert!(note_at > full_at, "notka musi być po pełnej turze: {wire}");
        assert!(wire.find("Stop, a pojutrze?") > note_at);
    }
}

/// Koszt z tabeli cen konfiguracji (nie z kodu).
pub async fn cost_comes_from_pricing<H: Harness>(h: &H) {
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
    let usage = Usage {
        input_tokens: 1_000_000,
        output_tokens: 1_000_000,
        ..Usage::default()
    };
    assert_eq!(p.cost(&h.model(), &usage), Some(h.pricing().cost(&usage)));
    assert_eq!(p.cost("model-bez-cennika-xyz", &usage), None);
    let est = p.estimate_cost(&request(h));
    assert!(
        est.is_some_and(|e| e.max >= e.min && e.input_tokens > 0),
        "{est:?}"
    );
}
