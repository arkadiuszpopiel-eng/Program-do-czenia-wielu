//! Zakończenie generacji: zapis tury agentki (append-only; odpowiedź, która nie powstała, jest
//! zapisywana jako komunikat systemowy), koszt w `cost-meter`, fakty tury, oś czasu,
//! zdarzenia końcowe i stan systemu (offline / 429 — przez rdzeń).

use accounts_hub_contract::{AccountId, ModelId, ProviderId};
use core_bus_contract::Cost;
use cost_meter_contract::{CostInput, Pricing, Usage as CostUsage};
use sessions_contract::{
    AgentId, Author, ModelUsage, NewTurn, Role, SessionHistory, Turn, TurnContent,
};

use app_api::AppError;
use app_api::dto::{
    self, AlfaEvent, EventLevel, Money, TimelineEvent, TimelineKind, TurnStatus, TurnUsage,
};
use app_api::ids;
use app_store::TurnMeta;

use crate::engine::{ChatEngine, GenHandle};
use crate::generate::{GenRequest, Placement};
use crate::outcome::{Chosen, Outcome};

/// Tekst tury-komunikatu, gdy odpowiedź nie powstała.
const NO_ANSWER: &str = "Odpowiedź przerwana przed pierwszym słowem.";

fn new_turn(req: &GenRequest, outcome: &Outcome) -> NewTurn {
    if outcome.text.trim().is_empty() {
        let message = outcome
            .error
            .as_ref()
            .map_or_else(|| NO_ANSWER.to_owned(), |e| e.message.clone());
        return NewTurn {
            role: Role::System,
            author: Author::System,
            content: TurnContent::text(message),
            usage: None,
            heard_prefix: None,
        };
    }
    let usage = match (&outcome.chosen, outcome.usage) {
        (Some(chosen), Some(u)) => Some(ModelUsage {
            provider: chosen.provider_id.clone(),
            model: chosen.model.clone(),
            cost: Cost {
                input_tokens: u.input_tokens,
                output_tokens: u.output_tokens,
                micro_usd: outcome.cost_nano_usd.map_or(0, |n| n.div_ceil(1000)),
                latency_ms: Some(outcome.latency_ms),
            },
        }),
        _ => None,
    };
    NewTurn {
        role: Role::Assistant,
        author: Author::Agent {
            agent: AgentId::new(req.agent.as_str()),
        },
        content: TurnContent {
            text: outcome.text.clone(),
            blocks: outcome.thinking.clone(),
        },
        usage,
        heard_prefix: None,
    }
}

impl ChatEngine {
    fn persist_turn(&self, req: &GenRequest, outcome: &Outcome) -> Result<Turn, AppError> {
        let turn = new_turn(req, outcome);
        match req.placement {
            Placement::Child(parent) => self.append_child(&req.session, Some(parent), turn),
            Placement::Sibling(of) => Ok(self.inner.sessions.fork_from(&req.session, of, turn)?),
        }
    }

    /// Jeden wpis kosztu; `None`, gdy identyfikatora dostawcy albo modelu nie da się zapisać.
    async fn record_one(
        &self,
        req: &GenRequest,
        chosen: &Chosen,
        usage: CostUsage,
        nano: Option<u64>,
    ) -> Option<Option<u64>> {
        let input = CostInput {
            session: Some(req.session.clone()),
            agent: Some(AgentId::new(req.agent.as_str())),
            provider: ProviderId::new(chosen.provider_id.as_str()).ok()?,
            account: chosen
                .account
                .as_deref()
                .and_then(|a| AccountId::new(a).ok()),
            model: ModelId::new(chosen.model.as_str()).ok()?,
            usage,
            pricing: match nano {
                Some(nano) => Pricing::Reported {
                    micro_usd: nano.div_ceil(1000),
                },
                None => Pricing::Unknown,
            },
            background: false,
        };
        Some(match self.inner.costs.record(input).await {
            Ok(record) => record.micro_pln,
            Err(e) => {
                tracing::warn!(error = %e, "zapis kosztu nie powiódł się");
                None
            }
        })
    }

    async fn record_cost(&self, req: &GenRequest, outcome: &Outcome) -> Option<TurnUsage> {
        let chosen = outcome.chosen.as_ref()?;
        let usage = outcome.usage?;
        // Przebieg agentki: tury pod faktycznymi dostawcami (limit konta je widzi), reszta pod `chosen`.
        let mut rest = CostUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
        };
        let mut rest_nano = outcome.cost_nano_usd;
        let mut micro_pln: Option<u64> = None;
        let mut add = |m: Option<u64>| {
            if let Some(m) = m {
                micro_pln = Some(micro_pln.unwrap_or(0).saturating_add(m));
            }
        };
        for part in &outcome.parts {
            let part_usage = CostUsage {
                input_tokens: part.input_tokens,
                output_tokens: part.output_tokens,
                ..CostUsage::default()
            };
            if let Some(m) = self
                .record_one(req, &part.chosen, part_usage, part.cost_nano_usd)
                .await
            {
                add(m);
                rest.input_tokens = rest.input_tokens.saturating_sub(part.input_tokens);
                rest.output_tokens = rest.output_tokens.saturating_sub(part.output_tokens);
                rest_nano = rest_nano
                    .map(|n| n.saturating_sub(part.cost_nano_usd.unwrap_or(0)))
                    .filter(|n| *n > 0);
            }
        }
        let rest_left = rest.input_tokens + rest.output_tokens > 0 || rest_nano.is_some();
        if outcome.parts.is_empty() || rest_left {
            match self.record_one(req, chosen, rest, rest_nano).await {
                Some(m) => add(m),
                None if outcome.parts.is_empty() => return None,
                None => {}
            }
        }
        // Jeden faktyczny dostawca bez reszty — tura pokazuje jego, nie „Router · auto”.
        let shown = match outcome.parts.as_slice() {
            [only] if !rest_left => &only.chosen,
            _ => chosen,
        };
        Some(TurnUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cost: Money::from_micro_pln(micro_pln.unwrap_or(0)),
            cost_micro_pln: micro_pln,
            latency_ms: outcome.latency_ms,
            provider: shown.provider_name.clone(),
            model: shown.model.clone(),
        })
    }

    fn timeline_model_call(
        &self,
        req: &GenRequest,
        turn_id: &str,
        outcome: &Outcome,
        usage: Option<&TurnUsage>,
    ) {
        let (title, level) = match (&outcome.chosen, usage, &outcome.error) {
            (_, _, Some(e)) => (format!("Błąd: {}", e.message), EventLevel::Error),
            (Some(c), Some(u), None) => (
                format!(
                    "{} · {} → {} tokenów",
                    c.model, u.input_tokens, u.output_tokens
                ),
                EventLevel::Info,
            ),
            (Some(c), None, None) => (c.model.clone(), EventLevel::Info),
            (None, _, None) => match outcome.local {
                Some(name) => (format!("Bez modelu: {name}"), EventLevel::Info),
                None => ("Wywołanie modelu".to_owned(), EventLevel::Info),
            },
        };
        let event = TimelineEvent {
            id: ids::timeline_dto(&req.session),
            ts: dto::iso(chrono::Utc::now()),
            session_id: req.session.to_string(),
            kind: TimelineKind::ModelCall,
            level,
            agent: Some(req.agent.clone()),
            title,
            detail: outcome.stop.map(|s| format!("{s:?}").to_lowercase()),
            cost: usage.map(|u| u.cost),
            latency_ms: Some(outcome.latency_ms),
            turn_id: Some(turn_id.to_owned()),
        };
        if let Err(e) = self.inner.store.push_timeline(&req.session, &event) {
            tracing::warn!(error = %e, "zapis osi czasu nie powiódł się");
        }
        self.emit(AlfaEvent::TimelineAppended { event });
    }

    /// Zapisuje wynik generacji i wysyła zdarzenia końcowe.
    pub(crate) async fn finish_generation(
        &self,
        req: &GenRequest,
        handle: &GenHandle,
        outcome: Outcome,
    ) {
        let sid = req.session.to_string();
        let tid = ids::turn_dto(&req.session, handle.turn);
        let usage = self.record_cost(req, &outcome).await;
        match self.persist_turn(req, &outcome) {
            Ok(turn) => {
                if turn.id != handle.turn {
                    tracing::error!(zarezerwowana = %handle.turn, zapisana = %turn.id, "niezgodny numer tury");
                }
                let meta = TurnMeta {
                    status: Some(outcome.status),
                    agent: Some(req.agent.clone()),
                    role_id: self.role_of(&req.session, &req.agent),
                    addressed_to: None,
                    continues: req.continues.map(|c| c.0),
                    truncated: outcome.stop == Some(dto::StopReason::MaxTokens),
                    thinking_ms: outcome.thinking_ms,
                    usage: usage.clone(),
                    error: outcome.error.clone(),
                    tools: outcome.tools.clone(),
                    approval: outcome.approval.clone(),
                };
                if let Err(e) = self.inner.store.put_meta(&req.session, turn.id, &meta) {
                    tracing::error!(error = %e, "zapis faktów tury nie powiódł się");
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "zapis tury agentki nie powiódł się");
                // Odpowiedź jest na ekranie, ale nie ma jej w historii — użytkownik musi to wiedzieć,
                // zanim zamknie okno (wcześniej tylko wpis w dzienniku).
                self.emit(AlfaEvent::Toast {
                    kind: dto::ToastKind::Error,
                    message: dto::LocalizedText::new(
                        "Odpowiedź nie została zapisana w historii — po ponownym uruchomieniu jej nie będzie. Skopiuj ją, jeśli jest potrzebna.",
                        "The reply was not saved to history — it will be gone after a restart. Copy it if you need it.",
                    ),
                });
            }
        }
        if let Some(usage) = &usage {
            self.emit(AlfaEvent::Usage {
                session_id: sid.clone(),
                turn_id: tid.clone(),
                usage: usage.clone(),
            });
        }
        // Stan łączności (offline / 429) przed zdarzeniem końca tury — kto po `Error` pyta
        // o `system_status`, widzi już limit dostawcy (wcześniej wyścig).
        if let Some(host) = self.host() {
            let answered = outcome.chosen.is_some() && outcome.status != TurnStatus::Error;
            host.connectivity(outcome.error.as_ref(), answered).await;
            let account = outcome.chosen.as_ref().and_then(|c| c.account.as_deref());
            if let (Some(account), Some(error)) = (account, &outcome.error) {
                host.account_trouble(account, error).await;
            }
        }
        match (&outcome.error, outcome.stop) {
            (Some(error), _) => self.emit(AlfaEvent::Error {
                session_id: sid.clone(),
                turn_id: tid.clone(),
                error: error.clone(),
            }),
            (None, Some(reason)) => self.emit(AlfaEvent::Stop {
                session_id: sid.clone(),
                turn_id: tid.clone(),
                reason,
            }),
            (None, None) => {}
        }
        if outcome.status != TurnStatus::Error || outcome.chosen.is_some() {
            self.timeline_model_call(req, &tid, &outcome, usage.as_ref());
        }
        {
            let mut rt = self.rt();
            if rt
                .gens
                .get(&req.session)
                .is_some_and(|g| g.turn == handle.turn)
            {
                rt.gens.remove(&req.session);
            }
        }
        self.emit(AlfaEvent::ActivityChanged {
            session_id: sid,
            activity: None,
        });
        self.announce_agents(&req.session);
        self.announce_session(&req.session).await;
        if let Some(host) = self.host() {
            host.costs_changed(&req.session).await;
        }
    }
}
