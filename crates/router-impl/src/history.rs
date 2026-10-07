//! Pochodzenie bloków myślenia przez Router.
//!
//! Konsument składa turę `TurnAccumulator::new(router.id())`, więc bloki myślenia mają
//! `provider_origin = {router, "dostawca:model"}` (Router zwraca w `Started` model kwalifikowany).
//! Przed wysłaniem do dostawcy Router przywraca rzeczywiste pochodzenie `{dostawca, model}` —
//! adapter tego samego dostawcy odeśle podpis bajt w bajt, obcy go pominie. Historia konsumenta
//! pozostaje nietknięta (zmieniana jest kopia żądania; append-only, ADR 0006).

use providers_contract::{ContentBlock, Message, ProviderEvent, ProviderId, ProviderOrigin};
use router_contract::Candidate;

fn localize_origin(origin: &mut ProviderOrigin, router: &ProviderId) {
    if &origin.provider != router {
        return;
    }
    if let Some(real) = Candidate::parse(&origin.model) {
        origin.provider = real.provider;
        origin.model = real.model;
    }
}

/// Kopia historii z przywróconym pochodzeniem bloków myślenia wytworzonych przez Router.
pub fn localize_history(messages: &[Message], router: &ProviderId) -> Vec<Message> {
    messages
        .iter()
        .map(|m| {
            let mut m = m.clone();
            for block in &mut m.content {
                match block {
                    ContentBlock::Thinking(t) => localize_origin(&mut t.provider_origin, router),
                    ContentBlock::RedactedThinking(r) => {
                        localize_origin(&mut r.provider_origin, router);
                    }
                    _ => {}
                }
            }
            m
        })
        .collect()
}

fn qualify(candidate: &Candidate, model: &str) -> String {
    let model = if model.is_empty() {
        candidate.model.as_str()
    } else {
        model
    };
    Candidate::new(candidate.provider.clone(), model).qualified()
}

/// Zdarzenie dla konsumenta: modele w `Started`/`ModelSwitched` jako `dostawca:model`.
pub fn qualify_event(event: ProviderEvent, candidate: &Candidate) -> ProviderEvent {
    match event {
        ProviderEvent::Started { model, response_id } => ProviderEvent::Started {
            model: qualify(candidate, &model),
            response_id,
        },
        ProviderEvent::ModelSwitched { from, to } => ProviderEvent::ModelSwitched {
            from: qualify(candidate, &from),
            to: qualify(candidate, &to),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use providers_contract::{RedactedThinkingBlock, Role, ThinkingBlock};

    #[test]
    fn origins_round_trip() {
        let router = ProviderId::new("router");
        let block = |provider: &str, model: &str| {
            ContentBlock::Thinking(ThinkingBlock {
                text: "t".into(),
                signature: Some("sig".into()),
                provider_origin: ProviderOrigin {
                    provider: ProviderId::new(provider),
                    model: model.into(),
                },
            })
        };
        let redacted = ContentBlock::RedactedThinking(RedactedThinkingBlock {
            data: "d".into(),
            provider_origin: ProviderOrigin {
                provider: router.clone(),
                model: "anthropic:claude-opus-5-5".into(),
            },
        });
        let msg = Message::new(
            Role::Assistant,
            vec![
                block("router", "anthropic:claude-opus-5-5"),
                block("openai", "gpt-6-sol"),
                block("router", "bez-kwalifikacji"),
                redacted,
            ],
        );
        let out = localize_history(std::slice::from_ref(&msg), &router);
        let origins: Vec<(String, String)> = out[0]
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Thinking(t) => Some(&t.provider_origin),
                ContentBlock::RedactedThinking(r) => Some(&r.provider_origin),
                _ => None,
            })
            .map(|o| (o.provider.to_string(), o.model.clone()))
            .collect();
        assert_eq!(
            origins,
            [
                ("anthropic".into(), "claude-opus-5-5".into()),
                ("openai".into(), "gpt-6-sol".into()),
                ("router".into(), "bez-kwalifikacji".into()),
                ("anthropic".into(), "claude-opus-5-5".into()),
            ]
        );
        assert_ne!(out[0], msg, "kopia zmieniona, oryginał nie");
    }

    #[test]
    fn events_are_qualified() {
        let c = Candidate::new("local", "bielik");
        let started = qualify_event(
            ProviderEvent::Started {
                model: String::new(),
                response_id: None,
            },
            &c,
        );
        assert_eq!(
            started,
            ProviderEvent::Started {
                model: "local:bielik".into(),
                response_id: None
            }
        );
        let switched = qualify_event(
            ProviderEvent::ModelSwitched {
                from: "a".into(),
                to: "b".into(),
            },
            &c,
        );
        assert_eq!(
            switched,
            ProviderEvent::ModelSwitched {
                from: "local:a".into(),
                to: "local:b".into()
            }
        );
        let text = ProviderEvent::TextDelta {
            index: 0,
            text: "x".into(),
        };
        assert_eq!(qualify_event(text.clone(), &c), text);
    }
}
