//! Zakończenie generacji: zapis tury agentki (append-only; odpowiedź, która nie powstała, jest
//! zapisywana jako komunikat systemowy), koszt w `cost-meter`, fakty `app-core`, oś czasu,
//! zdarzenia końcowe i stan systemu (offline / 429).

use accounts_hub_contract::{AccountId, ModelId, ProviderId};
use core_bus_contract::Cost;
use cost_meter_contract::{CostInput, CostMeter, Pricing, Usage as CostUsage};
use sessions_contract::{
    AgentId, Author, ModelUsage, NewTurn, Role, SessionHistory, Turn, TurnContent,
};

use super::stream::Outcome;
use super::{GenRequest, Placement};
use crate::core::{AppCore, GenHandle};
use crate::dto::{
    self, AlfaEvent, EventLevel, Money, TimelineEvent, TimelineKind, TurnErrorCode, TurnStatus,
    TurnUsage,
};
use crate::ids;
use app_store::TurnMeta;

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

impl AppCore {
    fn persist_turn(&self, req: &GenRequest, outcome: &Outcome) -> Result<Turn, crate::AppError> {
        let turn = new_turn(req, outcome);
        match req.placement {
            Placement::Child(parent) => self.append_child(&req.session, Some(parent), turn),
            Placement::Sibling(of) => Ok(self.inner.sessions.fork_from(&req.session, of, turn)?),
        }
    }

    async fn record_cost(&self, req: &GenRequest, outcome: &Outcome) -> Option<TurnUsage> {
        let chosen = outcome.chosen.as_ref()?;
        let usage = outcome.usage?;
        let input = CostInput {
            session: Some(req.session.clone()),
            agent: Some(AgentId::new(req.agent.as_str())),
            provider: ProviderId::new(chosen.provider_id.as_str()).ok()?,
            account: chosen
                .account
                .as_deref()
                .and_then(|a| AccountId::new(a).ok()),
            model: ModelId::new(chosen.model.as_str()).ok()?,
            usage: CostUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_read_tokens: usage.cache_read_tokens,
                cache_write_tokens: usage.cache_write_tokens,
            },
            pricing: match outcome.cost_nano_usd {
                Some(nano) => Pricing::Reported {
                    micro_usd: nano.div_ceil(1000),
                },
                None => Pricing::Unknown,
            },
            background: false,
        };
        let micro_pln = match self.inner.costs.record(input).await {
            Ok(record) => record.micro_pln.unwrap_or(0),
            Err(e) => {
                tracing::warn!(error = %e, "zapis kosztu nie powiódł się");
                0
            }
        };
        Some(TurnUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cost: Money::from_micro_pln(micro_pln),
            latency_ms: outcome.latency_ms,
            provider: chosen.provider_name.clone(),
            model: chosen.model.clone(),
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
            (None, _, None) => ("Wywołanie modelu".to_owned(), EventLevel::Info),
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
            Err(e) => tracing::error!(error = %e, "zapis tury agentki nie powiódł się"),
        }
        if let Some(usage) = &usage {
            self.emit(AlfaEvent::Usage {
                session_id: sid.clone(),
                turn_id: tid.clone(),
                usage: usage.clone(),
            });
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
        self.update_connectivity(&outcome).await;
        self.emit(AlfaEvent::ActivityChanged {
            session_id: sid.clone(),
            activity: None,
        });
        self.announce_agents(&req.session);
        self.announce_session(&req.session).await;
        let costs = self.cost_summary(Some(&req.session)).await;
        self.emit(AlfaEvent::CostsChanged {
            session_id: sid,
            costs,
        });
    }

    /// Offline / 429 z wyniku → stan systemu (zdarzenie tylko przy zmianie).
    async fn update_connectivity(&self, outcome: &Outcome) {
        let changed = {
            let mut rt = self.rt();
            let before = (rt.online, rt.rate_limit.clone());
            match outcome.error.as_ref().map(|e| e.code) {
                Some(TurnErrorCode::Offline) => rt.online = false,
                Some(TurnErrorCode::RateLimited) => {
                    let e = outcome.error.as_ref();
                    let at = e
                        .and_then(|e| e.retry_at.as_deref())
                        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                        .map(|t| t.with_timezone(&chrono::Utc));
                    let provider = e.and_then(|e| e.provider.clone()).unwrap_or_default();
                    rt.rate_limit = at.map(|at| (provider, at));
                }
                _ if outcome.chosen.is_some() && outcome.status != TurnStatus::Error => {
                    rt.online = true;
                    rt.rate_limit = None;
                }
                _ => {}
            }
            before != (rt.online, rt.rate_limit.clone())
        };
        if changed {
            let status = self.status_snapshot().await;
            self.emit(AlfaEvent::SystemStatusChanged { status });
        }
    }
}
