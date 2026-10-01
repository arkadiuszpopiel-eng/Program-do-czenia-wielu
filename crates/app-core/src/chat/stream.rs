//! Przebieg jednej generacji: wybór modelu → budżet → strumień dostawcy z renderowaniem
//! przyrostowym (`lib-markdown::IncrementalRenderer`) → wynik do zapisu.

use std::time::{Duration, Instant};

use cost_meter_contract::{BudgetDecision, CostMeter, usd_to_pln};
use futures_util::StreamExt;
use lib_markdown::{IncrementalRenderer, RenderOptions, StreamUpdate};
use personas_contract::{PersonaId, Personas};
use providers_contract::{
    ChatRequest, ProviderError, ProviderErrorKind, ProviderEvent, StopReason as PStop,
    TurnAccumulator, Usage,
};
use sessions_contract::{Block as SBlock, PrivacyTag, SessionCatalog};

use super::GenRequest;
use super::project::block_dto;
use crate::core::{AppCore, GenHandle};
use crate::dto::{
    self, AlfaEvent, RenderedBlock, StopReason, TurnError, TurnErrorCode, TurnStatus,
};
use crate::ports::{BrainError, BrainRequest};

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
    pub tools: Vec<crate::dto::ToolStep>,
    /// Karta „czeka na zatwierdzenie" (przebieg agentki).
    pub approval: Option<crate::dto::ApprovalPending>,
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

fn turn_error(
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

fn update_blocks(update: StreamUpdate) -> Vec<RenderedBlock> {
    let mut blocks: Vec<RenderedBlock> = update.closed.iter().map(|b| block_dto(b, true)).collect();
    if let Some(open) = &update.open {
        blocks.push(block_dto(open, false));
    }
    blocks
}

fn merge_live(live: &mut dto::Turn, text: &str, blocks: &[RenderedBlock]) {
    live.text.push_str(text);
    for b in blocks {
        match live.blocks.iter_mut().find(|x| x.index == b.index) {
            Some(x) => *x = b.clone(),
            None => live.blocks.push(b.clone()),
        }
    }
}

/// Przygotowanie żądania (historia gałęzi, prompt agentki, prywatność) i wybór trasy.
pub(crate) async fn prepare(
    core: &AppCore,
    req: &GenRequest,
) -> Result<(crate::ports::BrainChoice, ChatRequest), TurnError> {
    let privacy = core
        .inner
        .sessions
        .session(&req.session)
        .map(|m| m.privacy)
        .unwrap_or_default();
    let messages = core
        .branch_messages(&req.session, req.history_leaf, req.continues.is_some())
        .map_err(|e| turn_error(TurnErrorCode::Provider, e.message, None))?;
    let mut request = ChatRequest::new(router_contract::AUTO_MODEL, messages);
    if let Ok(system) = core
        .inner
        .personas
        .system_prompt(&req.session, &PersonaId::new(req.agent.as_str()))
    {
        request = request.with_system(system);
    }
    request.meta.session = Some(req.session.to_string());
    request.meta.privacy.tag = match privacy {
        PrivacyTag::Normal => providers_contract::PrivacyTag::Normal,
        PrivacyTag::Private | PrivacyTag::LocalOnly => providers_contract::PrivacyTag::Private,
    };
    let brain_req = BrainRequest {
        session: req.session.clone(),
        agent: req.agent.clone(),
        profile: req.profile,
        privacy,
        chat: Some(request.clone()),
    };
    let choice = core
        .inner
        .brain
        .choose(&brain_req)
        .await
        .map_err(|e| match e {
            BrainError::NoKeys(m) => turn_error(TurnErrorCode::NoKeys, m, None),
            BrainError::Provider(m) => turn_error(TurnErrorCode::Provider, m, None),
            BrainError::Budget(m) => turn_error(TurnErrorCode::BudgetBlocked, m, None),
        })?;
    request.model.clone_from(&choice.model);
    if !choice.routed {
        budget_gate(core, &choice, &request).await?;
    }
    Ok((choice, request))
}

/// Budżet dla dostawcy wybranego poza Routerem (Router sprawdza go sam, per kandydat).
async fn budget_gate(
    core: &AppCore,
    choice: &crate::ports::BrainChoice,
    request: &ChatRequest,
) -> Result<(), TurnError> {
    let estimate = choice
        .provider
        .estimate_cost(request)
        .map_or(0, |e| e.min.micro_usd_ceil());
    let rate = core.inner.costs.current_rate().rate_e4;
    let provider_id = accounts_hub_contract::ProviderId::new(choice.provider_id.as_str()).ok();
    let decision = core
        .inner
        .costs
        .check_budget(usd_to_pln(estimate, rate), false, provider_id.as_ref())
        .await;
    if let BudgetDecision::Block { notice } = decision {
        return Err(turn_error(
            TurnErrorCode::BudgetBlocked,
            format!(
                "Limit kosztów osiągnięty ({} %) — zmień limit w Ustawieniach → Koszty.",
                notice.pct_after
            ),
            Some(&choice.provider_name),
        ));
    }
    Ok(())
}

/// Pełny przebieg generacji.
pub(crate) async fn generate(core: &AppCore, req: &GenRequest, handle: &GenHandle) -> Outcome {
    let (choice, request) = match prepare(core, req).await {
        Ok(x) => x,
        Err(e) => return Outcome::failed(e),
    };
    let mut chosen = Chosen {
        provider_id: choice.provider_id.clone(),
        provider_name: choice.provider_name.clone(),
        account: choice.account.clone(),
        model: choice.model.clone(),
    };
    let mut announced = false;
    if let Some(window) = choice.context_window {
        core.rt().context_window.insert(req.session.clone(), window);
    }
    let sid = req.session.to_string();
    let tid = handle.live.lock().map(|l| l.id.clone()).unwrap_or_default();
    let started = Instant::now();
    let mut stream = choice.provider.stream(request, handle.cancel.clone());
    let mut acc = TurnAccumulator::new(choice.provider.id().clone());
    let mut renderer = IncrementalRenderer::new(RenderOptions::default());
    let mut thinking: Option<(Instant, Instant)> = None;
    let mut thinking_ms = None;
    let mut cancelled = false;
    loop {
        let event = tokio::select! {
            biased;
            () = handle.cancel.cancelled() => { cancelled = true; break; }
            next = stream.next() => match next { Some(e) => e, None => break },
        };
        acc.push(&event);
        match &event {
            ProviderEvent::Started { model, .. } if choice.routed && !announced => {
                announced = true;
                if let Some(target) = core.inner.brain.target(model) {
                    super::routing::announce(core, req, &tid, &choice, &target);
                    chosen = super::routing::chosen_of(&target);
                }
            }
            ProviderEvent::ThinkingDelta { .. } => {
                let now = Instant::now();
                let (start, last) = thinking.get_or_insert((now, now - Duration::from_secs(1)));
                if now.duration_since(*last) >= Duration::from_millis(250) {
                    *last = now;
                    core.emit(AlfaEvent::ThinkingDelta {
                        session_id: sid.clone(),
                        turn_id: tid.clone(),
                        elapsed_ms: millis(now.duration_since(*start)),
                        done: false,
                    });
                }
            }
            ProviderEvent::TextDelta { text, .. } => {
                if let Some((start, _)) = thinking.take() {
                    let ms = millis(start.elapsed());
                    thinking_ms = Some(ms);
                    core.emit(AlfaEvent::ThinkingDelta {
                        session_id: sid.clone(),
                        turn_id: tid.clone(),
                        elapsed_ms: ms,
                        done: true,
                    });
                }
                if let Some(tap) = &req.tap {
                    let _ = tap.send(crate::ports::VoiceChunk::Text(text.clone()));
                }
                let blocks = update_blocks(renderer.push(text));
                if let Ok(mut live) = handle.live.lock() {
                    merge_live(&mut live, text, &blocks);
                }
                core.emit(AlfaEvent::TextDelta {
                    session_id: sid.clone(),
                    turn_id: tid.clone(),
                    text: text.clone(),
                    blocks,
                });
            }
            _ => {}
        }
    }
    drop(stream);
    let source = renderer.source().to_owned();
    let closing = update_blocks(renderer.finish());
    if !closing.is_empty() {
        core.emit(AlfaEvent::TextDelta {
            session_id: sid.clone(),
            turn_id: tid,
            text: String::new(),
            blocks: closing,
        });
    }
    let turn = acc.finish();
    let mut text = turn.message.visible_text();
    if text.is_empty() {
        text = source;
    }
    let thinking_blocks = turn
        .message
        .content
        .iter()
        .filter_map(|b| match b {
            providers_contract::ContentBlock::Thinking(t) => Some(SBlock::Thinking {
                provider: chosen.provider_id.clone(),
                text: t.text.clone(),
                signature: t.signature.clone(),
            }),
            _ => None,
        })
        .collect();
    let cost = turn
        .model
        .as_deref()
        .and_then(|m| choice.provider.cost(m, &turn.usage))
        .or_else(|| choice.provider.cost(&chosen.model, &turn.usage));
    let (status, stop, error) = classify(cancelled, &turn, &chosen.provider_name);
    if let Some(model) = turn.model.clone().filter(|_| !choice.routed) {
        chosen.model = model;
    }
    Outcome {
        text,
        thinking: thinking_blocks,
        status,
        stop,
        error,
        usage: (turn.usage != Usage::default()).then_some(turn.usage),
        cost_nano_usd: cost.map(|c| c.nano_usd),
        chosen: Some(chosen),
        latency_ms: millis(started.elapsed()),
        thinking_ms: thinking_ms.or_else(|| thinking.map(|(s, _)| millis(s.elapsed()))),
        tools: Vec::new(),
        approval: None,
    }
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

fn classify(
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
