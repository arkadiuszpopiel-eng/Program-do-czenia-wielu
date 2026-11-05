//! Model-sędzia na `ModelProvider` (`docs/modules/approval-judge/SPEC.md`, „Model i prompt”).
//!
//! Jedno zapytanie bez narzędzi, temperatura 0, odpowiedź tylko JSON. Termin, błąd dostawcy, odmowa
//! modelu, ucięta albo nieczytelna odpowiedź → `Escalate` (decyduje właściciel). Nigdy cicha zgoda.

use std::sync::Arc;
use std::time::Duration;

use approval_judge_contract::{
    ApprovalJudge, JudgeInput, JudgeVerdict, parse_verdict, render_prompt,
};
use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, Message, ModelProvider, ProviderEvent, RouteHint, StopReason,
};

mod driver;

pub use driver::{JudgeDriver, Judges, POLL_EVERY, build_input};

/// Manifest modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Najdłuższa odpowiedź sędziego (tokeny) — sam JSON z krótkim powodem.
const MAX_OUTPUT_TOKENS: u32 = 300;
/// Najdłuższy zebrany tekst odpowiedzi (znaki) — dłuższy to nie werdykt.
const MAX_REPLY_CHARS: usize = 4_000;

/// Ustawienia sędziego.
#[derive(Debug, Clone)]
pub struct JudgeConfig {
    /// Model u dostawcy (`auto` dla Routera albo przypięty, np. `local:bielik-4.5b`).
    pub model: String,
    /// Termin jednej oceny.
    pub timeout: Duration,
    /// Limit wejścia w tokenach (≈ 4 znaki na token).
    pub max_input_tokens: usize,
}

impl Default for JudgeConfig {
    fn default() -> Self {
        Self {
            model: "auto".into(),
            timeout: Duration::from_secs(25),
            max_input_tokens: 6_000,
        }
    }
}

/// Sędzia na dostawcy modeli.
pub struct ModelJudge {
    provider: Arc<dyn ModelProvider>,
    config: JudgeConfig,
}

impl ModelJudge {
    /// Nowy sędzia.
    pub fn new(provider: Arc<dyn ModelProvider>, config: JudgeConfig) -> Self {
        Self { provider, config }
    }

    fn request(&self, input: &JudgeInput) -> ChatRequest {
        let prompt = render_prompt(input, self.config.max_input_tokens.saturating_mul(4));
        let mut req = ChatRequest::new(
            self.config.model.clone(),
            vec![Message::user_text(prompt.user)],
        )
        .with_system(prompt.system);
        req.params.temperature = Some(0.0);
        req.params.max_tokens = Some(MAX_OUTPUT_TOKENS);
        req.cache.enabled = false;
        req.meta.route = RouteHint {
            review: true,
            ..RouteHint::default()
        };
        req
    }

    async fn ask(&self, req: ChatRequest, cancel: CancellationToken) -> JudgeVerdict {
        let mut stream = self.provider.stream(req, cancel);
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event {
                ProviderEvent::TextDelta { text: t, .. } => {
                    text.push_str(&t);
                    if text.chars().count() > MAX_REPLY_CHARS {
                        return JudgeVerdict::escalate("odpowiedź sędziego za długa");
                    }
                }
                ProviderEvent::Stop {
                    reason: StopReason::EndTurn | StopReason::StopSequence,
                    ..
                } => return parse_verdict(&text),
                ProviderEvent::Stop { reason, .. } => {
                    return JudgeVerdict::escalate(format!(
                        "sędzia nie dokończył odpowiedzi ({reason:?})"
                    ));
                }
                ProviderEvent::Error(e) => {
                    return JudgeVerdict::escalate(format!("błąd modelu sędziego: {e}"));
                }
                _ => {}
            }
        }
        JudgeVerdict::escalate("strumień sędziego urwał się bez końca odpowiedzi")
    }
}

#[async_trait::async_trait]
impl ApprovalJudge for ModelJudge {
    async fn review(&self, input: &JudgeInput) -> JudgeVerdict {
        let req = self.request(input);
        if let Err(e) = req.validate() {
            return JudgeVerdict::escalate(format!("niepoprawne zapytanie sędziego: {e}"));
        }
        let cancel = CancellationToken::new();
        // Upuszczenie strumienia po terminie przerywa połączenie (kontrakt `ModelProvider::stream`).
        match tokio::time::timeout(self.config.timeout, self.ask(req, cancel.clone())).await {
            Ok(verdict) => verdict,
            Err(_) => {
                cancel.cancel();
                JudgeVerdict::escalate("sędzia nie zdążył z oceną")
            }
        }
    }

    fn model_name(&self) -> String {
        format!("{}:{}", self.provider.id(), self.config.model)
    }
}
