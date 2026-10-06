//! Przebieg jednej generacji: wybór modelu → budżet → strumień dostawcy z renderowaniem
//! przyrostowym (`lib-markdown::IncrementalRenderer`) → wynik do zapisu.

use std::time::{Duration, Instant};

use cost_meter_contract::{BudgetDecision, usd_to_pln};
use futures_util::StreamExt;
use lib_markdown::{IncrementalRenderer, RenderOptions, StreamUpdate};
use personas_contract::PersonaId;
use providers_contract::{ChatRequest, ProviderEvent, TurnAccumulator, Usage};
use sessions_contract::{Block as SBlock, PrivacyTag, SessionCatalog};

use app_api::dto::{self, AlfaEvent, RenderedBlock, TurnError, TurnErrorCode};
use app_api::ports::{BrainChoice, BrainError, BrainRequest, VoiceChunk};

use crate::engine::{ChatEngine, GenHandle};
use crate::generate::GenRequest;
use crate::outcome::{Chosen, Outcome, classify, millis, turn_error};
use crate::project::block_dto;

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
    core: &ChatEngine,
    req: &GenRequest,
) -> Result<(BrainChoice, ChatRequest), TurnError> {
    let privacy = core
        .inner
        .sessions
        .session(&req.session)
        .map(|m| m.privacy)
        .unwrap_or_default();
    let messages = core
        .branch_messages(&req.session, req.history_leaf, req.continues.is_some())
        .map_err(|e| turn_error(TurnErrorCode::Provider, e.message, None))?;
    let query = messages.last().map(|m| m.visible_text());
    let mut request = ChatRequest::new(router_contract::AUTO_MODEL, messages);
    if let Ok(system) = core
        .inner
        .personas
        .system_prompt(&req.session, &PersonaId::new(req.agent.as_str()))
    {
        // Zestaw roboczy pamięci agentki (zakresy z obsady i projektu sesji) — jako dane.
        let memory = &core.inner.memory;
        let system = memory.system_prompt(system, &req.session, &req.agent, query);
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
    // Symptomy dostawcy dla Diagnosty (błąd HTTP → `diagnostics.symptom` bez treści rozmowy).
    let choice = app_health::symptom_tap(choice, &core.inner.bus);
    if !choice.routed {
        budget_gate(core, &choice, &request).await?;
    }
    Ok((choice, request))
}

/// Budżet dla dostawcy wybranego poza Routerem (Router sprawdza go sam, per kandydat).
async fn budget_gate(
    core: &ChatEngine,
    choice: &BrainChoice,
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
pub(crate) async fn generate(core: &ChatEngine, req: &GenRequest, handle: &GenHandle) -> Outcome {
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
                    crate::routing::announce(core, req, &tid, &choice, &target);
                    chosen = crate::routing::chosen_of(&target);
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
                    let _ = tap.send(VoiceChunk::Text(text.clone()));
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
