//! Skrypty odpowiedzi atrapy: sekwencje zdarzeń i opóźnień (wirtualny zegar tokio).

use std::time::Duration;

use providers_contract::{
    ProviderError, ProviderEvent, StopDetails, StopReason, ToolArguments, Usage,
    classify_http_status,
};

/// Krok skryptu.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Wyemituj zdarzenie.
    Emit(ProviderEvent),
    /// Odczekaj (w testach z `start_paused = true` — czas wirtualny, deterministyczny).
    Delay(Duration),
    /// Milcz do anulowania albo do limitu `first_token`/`idle` atrapy (wtedy `Timeout`).
    Stall,
}

/// Prędkość strumienia: czas do pierwszego tokenu i tokeny na sekundę (1 fragment = 1 token).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamSpeed {
    /// Czas do pierwszego tokenu.
    pub ttft: Duration,
    /// Tokeny na sekundę (> 0).
    pub tokens_per_sec: f64,
}

impl StreamSpeed {
    fn interval(&self) -> Duration {
        if self.tokens_per_sec > 0.0 && self.tokens_per_sec.is_finite() {
            Duration::from_secs_f64(1.0 / self.tokens_per_sec)
        } else {
            Duration::ZERO
        }
    }
}

/// Skrypt jednej odpowiedzi.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Script {
    /// Kroki w kolejności.
    pub steps: Vec<Step>,
}

fn started(model: &str) -> Step {
    Step::Emit(ProviderEvent::Started {
        model: model.to_owned(),
        response_id: Some("fake-resp".into()),
    })
}

fn usage(output_tokens: u64) -> Step {
    Step::Emit(ProviderEvent::Usage(Usage {
        input_tokens: 10,
        output_tokens: output_tokens.max(1),
        ..Usage::default()
    }))
}

fn stop(reason: StopReason) -> Step {
    Step::Emit(ProviderEvent::stop(reason))
}

fn text_steps(chunks: &[String], index: u32, gap: Duration) -> Vec<Step> {
    let mut steps = Vec::with_capacity(chunks.len() * 2);
    for (i, chunk) in chunks.iter().enumerate() {
        if i > 0 && !gap.is_zero() {
            steps.push(Step::Delay(gap));
        }
        steps.push(Step::Emit(ProviderEvent::TextDelta {
            index,
            text: chunk.clone(),
        }));
    }
    steps
}

impl Script {
    /// Skrypt z kroków.
    pub fn new(steps: Vec<Step>) -> Self {
        Self { steps }
    }

    /// Tekst w podanych fragmentach, natychmiast.
    pub fn text(model: &str, chunks: &[&str]) -> Self {
        let chunks: Vec<String> = chunks.iter().map(|c| (*c).to_owned()).collect();
        Self::chunks(model, &chunks, Duration::ZERO)
    }

    /// Tekst we fragmentach z odstępem `gap` między nimi.
    pub fn chunks(model: &str, chunks: &[String], gap: Duration) -> Self {
        let mut steps = vec![started(model)];
        steps.extend(text_steps(chunks, 0, gap));
        steps.push(usage(chunks.len() as u64));
        steps.push(stop(StopReason::EndTurn));
        Self { steps }
    }

    /// Tekst dzielony na słowa z zadaną prędkością (TTFT + tokeny/s).
    pub fn streamed(model: &str, text: &str, speed: StreamSpeed) -> Self {
        let words: Vec<String> = text.split_inclusive(' ').map(str::to_owned).collect();
        let mut steps = vec![Step::Delay(speed.ttft), started(model)];
        steps.extend(text_steps(&words, 0, speed.interval()));
        steps.push(usage(words.len() as u64));
        steps.push(stop(StopReason::EndTurn));
        Self { steps }
    }

    /// Jedno wywołanie narzędzia.
    pub fn tool_call(model: &str, id: &str, name: &str, arguments: &serde_json::Value) -> Self {
        let raw = arguments.to_string();
        let (a, b) = raw.split_at(raw.len() / 2);
        Self::new(vec![
            started(model),
            Step::Emit(ProviderEvent::ToolCallStart {
                index: 0,
                id: id.to_owned(),
                name: name.to_owned(),
            }),
            Step::Emit(ProviderEvent::ToolCallDelta {
                index: 0,
                partial_json: a.to_owned(),
            }),
            Step::Emit(ProviderEvent::ToolCallDelta {
                index: 0,
                partial_json: b.to_owned(),
            }),
            Step::Emit(ProviderEvent::ToolCallEnd {
                index: 0,
                id: id.to_owned(),
                arguments: ToolArguments::from_raw(&raw),
            }),
            usage(5),
            stop(StopReason::ToolUse),
        ])
    }

    /// Myślenie z podpisem, potem tekst.
    pub fn thinking(model: &str, thinking: &str, signature: &str, text: &str) -> Self {
        Self::new(vec![
            started(model),
            Step::Emit(ProviderEvent::ThinkingDelta {
                index: 0,
                text: thinking.to_owned(),
            }),
            Step::Emit(ProviderEvent::ThinkingSignature {
                index: 0,
                signature: signature.to_owned(),
            }),
            Step::Emit(ProviderEvent::TextDelta {
                index: 1,
                text: text.to_owned(),
            }),
            usage(3),
            stop(StopReason::EndTurn),
        ])
    }

    /// Błąd końcowy (np. wstrzyknięty 429/5xx/timeout).
    pub fn error(error: ProviderError) -> Self {
        Self::new(vec![Step::Emit(ProviderEvent::Error(error))])
    }

    /// Błąd HTTP sklasyfikowany jak w adapterach.
    pub fn http_error(status: u16, retry_after_s: Option<u64>) -> Self {
        let kind = classify_http_status(status, retry_after_s.map(|s| s * 1000));
        Self::error(ProviderError::new(kind, format!("atrapa: HTTP {status}")).with_status(status))
    }

    /// Odmowa (kategoria `cyber`).
    pub fn refusal(model: &str) -> Self {
        Self::new(vec![
            started(model),
            usage(1),
            Step::Emit(ProviderEvent::Stop {
                reason: StopReason::Refusal,
                details: Some(StopDetails {
                    category: Some("cyber".into()),
                    ..StopDetails::default()
                }),
            }),
        ])
    }

    /// Tekst ucięty na `max_tokens`.
    pub fn max_tokens(model: &str, text: &str) -> Self {
        let mut s = Self::text(model, &[text]);
        if let Some(last) = s.steps.last_mut() {
            *last = stop(StopReason::MaxTokens);
        }
        s
    }

    /// Milczenie (bez `Started`).
    pub fn stall() -> Self {
        Self::new(vec![Step::Stall])
    }

    /// Dokleja opóźnienie na początku (np. sztuczne TTFT).
    pub fn delayed(mut self, by: Duration) -> Self {
        self.steps.insert(0, Step::Delay(by));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_end_with_terminal() {
        for s in [
            Script::text("m", &["a", "b"]),
            Script::tool_call("m", "t", "n", &serde_json::json!({"x": 1})),
            Script::thinking("m", "t", "s", "x"),
            Script::http_error(529, None),
            Script::refusal("m"),
            Script::max_tokens("m", "x"),
        ] {
            assert!(
                matches!(s.steps.last(), Some(Step::Emit(e)) if e.is_terminal()),
                "{s:?}"
            );
        }
        assert_eq!(Script::stall().steps, vec![Step::Stall]);
    }

    #[test]
    fn streamed_uses_speed() {
        let s = Script::streamed(
            "m",
            "raz dwa trzy",
            StreamSpeed {
                ttft: Duration::from_millis(200),
                tokens_per_sec: 10.0,
            },
        );
        assert_eq!(s.steps[0], Step::Delay(Duration::from_millis(200)));
        let delays = s
            .steps
            .iter()
            .filter(|st| matches!(st, Step::Delay(_)))
            .count();
        assert_eq!(delays, 3, "TTFT + 2 odstępy");
        let zero = StreamSpeed {
            ttft: Duration::ZERO,
            tokens_per_sec: 0.0,
        };
        assert_eq!(zero.interval(), Duration::ZERO);
    }
}
