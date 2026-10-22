//! Przebieg jednej generacji: wybór modelu → budżet → strumień dostawcy z renderowaniem
//! przyrostowym (`lib-markdown::IncrementalRenderer`) → wynik do zapisu.

use std::time::{Duration, Instant};

use cost_meter_contract::{BudgetDecision, usd_to_pln};
use futures_util::StreamExt;
use lib_markdown::{IncrementalRenderer, RenderOptions, StreamUpdate};
use personas_contract::PersonaId;
use providers_contract::{ChatRequest, ProviderEvent, TurnAccumulator, Usage};
use sessions_contract::{Block as SBlock, PrivacyTag, SessionCatalog};

use app_api::dto::{self, AlfaEvent, AutonomyLevel, RenderedBlock, TurnError, TurnErrorCode};
use app_api::ports::{BrainChoice, BrainError, BrainRequest, VoiceChunk};

use crate::engine::{ChatEngine, GenHandle};
use crate::generate::GenRequest;
use crate::outcome::{Chosen, Outcome, classify, millis, turn_error};
use crate::project::block_dto;
use crate::style;

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

/// Długość tekstu w jednostkach UTF-16 (tak liczy ją interfejs: `String.length`).
fn utf16_len(text: &str) -> u64 {
    text.encode_utf16().count() as u64
}

/// Kiedy Broker pyta właściciela o zgodę na danym poziomie — z tej samej tabeli reguł, którą
/// stosuje Broker (`risk_classifier_contract::RULES`; przegląd 21.10: skróty ze strony Uprawnień
/// pomijały m.in. operacje administracyjne i wysyłkę po niezaufanej treści na L4).
fn autonomy_note(level: AutonomyLevel) -> String {
    use risk_classifier_contract::{AutonomyLevel as Level, RULES, RuleId};
    let at = match level {
        AutonomyLevel::L0 => {
            return "L0 — agentka tylko czyta i podpowiada, nic nie zmienia".into();
        }
        AutonomyLevel::L1 => Level::L1,
        AutonomyLevel::L2 => Level::L2,
        AutonomyLevel::L3 => Level::L3,
        AutonomyLevel::L4 => Level::L4,
    };
    let asks: Vec<&str> = RULES
        .iter()
        .filter(|r| r.id != RuleId::KernelBlock && r.applies_at(at))
        .map(|r| r.description_pl)
        .collect();
    format!(
        "{at:?} — Broker pyta właściciela o zgodę, gdy: {}; twarde blokady Jądra obowiązują zawsze",
        asks.join("; ")
    )
}

/// Informacja w prompcie: kto w obsadzie działa na komputerze, jak go wywołać i kiedy Broker
/// pyta o zgodę (handoff 20.10: na L4 model zapowiadał zatwierdzanie każdej akcji).
fn crew_note(crew: &[String], level: Option<AutonomyLevel>) -> String {
    let names = crew.join(", ");
    let first = crew.first().map_or("Delta", String::as_str);
    let mut note = format!(
        "W tej rozmowie na komputerze użytkownika działa: {names} — pliki, programy, polecenia, \
         system, mysz i klawiatura, przez Brokera. Gdy prośba wymaga takiej pracy, a sama nie masz \
         narzędzi, nie mów, że nikt nie ma dostępu: powiedz, że zrobi to {first}, i podpowiedz, \
         żeby użytkownik napisał „{first}, …” albo wybrał ją w polu adresatki."
    );
    if let Some(level) = level {
        note.push_str(&format!(
            " Poziom autonomii tej rozmowy: {}. Mówiąc o zgodach, trzymaj się tego opisu: nie \
             zapowiadaj zgody na każdą akcję, ale też nie obiecuj, że jej nie będzie — o każdej \
             akcji decyduje Broker.",
            autonomy_note(level)
        ));
    }
    note
}

/// Przygotowanie żądania (historia gałęzi, prompt agentki, prywatność) i wybór trasy.
/// `pin` — model przypięty przez użytkownika (`dostawca:model`, Arena): Router próbuje go
/// pierwszego, z tymi samymi kontrolami prywatności, zgodności i budżetu co trasa automatyczna.
pub(crate) async fn prepare(
    core: &ChatEngine,
    req: &GenRequest,
    pin: Option<&str>,
) -> Result<(BrainChoice, ChatRequest), TurnError> {
    let privacy = core
        .inner
        .sessions
        .session(&req.session)
        .map(|m| m.privacy)
        .unwrap_or_default();
    // Projekcja historii czyta załączniki i wyciąga z nich tekst (PDF spoza pamięci podręcznej:
    // do ~11 s na plik) — poza wątkami runtime, żeby nie blokować innych zadań.
    let (engine, session, leaf) = (core.clone(), req.session.clone(), req.history_leaf);
    let continues = req.continues.is_some();
    let messages =
        tokio::task::spawn_blocking(move || engine.branch_messages(&session, leaf, continues))
            .await
            .map_err(|e| turn_error(TurnErrorCode::Provider, e.to_string(), None))?
            .map_err(|e| turn_error(TurnErrorCode::Provider, e.message, None))?;
    let query = messages.last().map(|m| m.visible_text());
    let mut request = ChatRequest::new(pin.unwrap_or(router_contract::AUTO_MODEL), messages);
    let mut system = None;
    if let Ok(persona) = core
        .inner
        .personas
        .system_prompt(&req.session, &PersonaId::new(req.agent.as_str()))
    {
        // Zestaw roboczy pamięci agentki (zakresy z obsady i projektu sesji) — jako dane.
        let memory = &core.inner.memory;
        system = Some(memory.system_prompt(persona, &req.session, &req.agent, query));
    }
    // Agentka bez narzędzi (Dyrygentka) wie, kto w obsadzie działa na komputerze — zamiast
    // odpowiadać, że dostępu nie ma (zgłoszenie właściciela 2026-10-20).
    let crew = core.crew_with_tools(&req.session, &req.agent);
    if !crew.is_empty() {
        // Poziom tylko przy działającym łączu: zerwane daje bezpieczne L0 (odmowa wszystkiego),
        // które nie jest poziomem rozmowy (przegląd 21.10).
        let level = match core.inner.broker.status().state {
            dto::BrokerLinkState::Connected => core
                .inner
                .broker
                .levels(Some(&req.session))
                .await
                .map(|v| v.session.unwrap_or(v.global)),
            _ => None,
        };
        let note = crew_note(&crew, level);
        system = Some(match system {
            Some(s) => format!("{s}\n\n{note}"),
            None => note,
        });
    }
    // Styl odpowiedzi i temperatura z ustawień (P4).
    let setting = |v: Option<serde_json::Value>| {
        v.and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    };
    let style = setting(core.config_value(style::STYLE_KEY).await);
    if let Some(system) = style::with_style(system, &style) {
        request = request.with_system(system);
    }
    request.params.temperature =
        style::temperature(&setting(core.config_value(style::TEMPERATURE_KEY).await));
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
        task: core.model_policy_of(&req.session, &req.agent),
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
        // Limit konta (od 0.0.70 egzekwowany) zmienia się przy koncie, nie w „Kosztach”.
        let place = match notice.scope {
            cost_meter_contract::BudgetScope::Provider(_) => {
                "limit konta w Ustawieniach → Konta i klucze API"
            }
            _ => "limit w Ustawieniach → Koszty",
        };
        return Err(turn_error(
            TurnErrorCode::BudgetBlocked,
            format!(
                "Limit kosztów osiągnięty ({} %) — zmień {place}.",
                notice.pct_after
            ),
            Some(&choice.provider_name),
        ));
    }
    Ok(())
}

/// Pełny przebieg generacji.
pub(crate) async fn generate(
    core: &ChatEngine,
    req: &GenRequest,
    handle: &GenHandle,
    pin: Option<&str>,
) -> Outcome {
    let (choice, request) = match prepare(core, req, pin).await {
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
    // Długość tekstu wysłanego w deltach tej tury (pole `offset` zdarzenia `TextDelta`).
    let mut sent_units = handle.live.lock().map(|l| utf16_len(&l.text)).unwrap_or(0);
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
                let offset = sent_units;
                sent_units += utf16_len(text);
                if let Ok(mut live) = handle.live.lock() {
                    merge_live(&mut live, text, &blocks);
                }
                core.emit(AlfaEvent::TextDelta {
                    session_id: sid.clone(),
                    turn_id: tid.clone(),
                    text: text.clone(),
                    blocks,
                    offset,
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
            offset: sent_units,
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
    // Błąd przed pierwszym zdarzeniem trasy: konto nie jest jeszcze znane — bierzemy je z decyzji
    // Routera, żeby rdzeń wiedział, które konto zawiodło (konto zapasowe, zadanie C2.15).
    if error.is_some() && chosen.account.is_none() {
        chosen.account = choice
            .route
            .as_ref()
            .and_then(|route| core.inner.brain.target(&route.chosen))
            .and_then(|target| target.account);
    }
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
        parts: Vec::new(),
        latency_ms: millis(started.elapsed()),
        thinking_ms: thinking_ms.or_else(|| thinking.map(|(s, _)| millis(s.elapsed()))),
        tools: Vec::new(),
        approval: None,
        local: None,
    }
}
