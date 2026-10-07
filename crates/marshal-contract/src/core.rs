//! Rdzeń Marszałka z otoczeniem ([`MarshalHost`]) i trait [`Marshal`] — wspólny dla `-impl`
//! i `-fake`. Tłumaczenie polecenia (LLM) idzie przez port [`RuleTranslator`]; jego wynik to
//! niezaufane szkice, które deterministyczny sprawdzacz przepuszcza tylko, gdy zawężają.

use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use chrono::NaiveDate;
use core_bus_contract::Event;
use serde_json::{Value, json};

use crate::book::{
    Approver, MAX_DECIDED_PROPOSALS, MAX_PENDING_PROPOSALS, MarshalError, Proposal, RuleBook,
};
use crate::check::Ceiling;
use crate::events::{
    EVENT_APPROVED, EVENT_PROPOSED, EVENT_REJECTED, EVENT_REPORT, EVENT_REVOKED, escalation_event,
    marshal_event,
};
use crate::policy::EffectivePolicy;
use crate::rule::{Rule, RuleId};
use crate::watch::{DailyReport, Escalation, Watch};

/// Port tłumacza: polecenie w języku naturalnym → szkice reguł (JSON, niezaufane).
#[async_trait]
pub trait RuleTranslator: Send + Sync {
    /// Szkice reguł dla polecenia; `ceiling` — kontekst (model ma proponować tylko zawężenia).
    async fn translate(&self, text: &str, ceiling: &Ceiling) -> Result<Vec<Value>, String>;
}

/// Otoczenie Marszałka.
pub trait MarshalHost: Send + Sync + 'static {
    /// Czas (ms UTC).
    fn now_ms(&self) -> u64;
    /// Zdarzenia.
    fn emit(&self, events: Vec<Event>);
    /// Trwały zapis księgi reguł.
    fn persist(&self, _book: &RuleBook) {}
}

/// Marszałek (API dla UI, Dyrygentki, poleceń głosowych).
#[async_trait]
pub trait Marshal: Send + Sync {
    /// Polecenie → propozycja reguł (przez tłumacza; czeka na decyzję użytkownika).
    async fn propose(&self, text: &str) -> Result<Proposal, MarshalError>;
    /// Propozycja ze szkiców (edytor reguł w UI, import `.alfa`).
    fn propose_drafts(&self, text: &str, drafts: Vec<Value>) -> Proposal;
    /// Zatwierdzenie — wyłącznie użytkownik.
    fn approve(&self, proposal: u64, approver: Approver) -> Result<Vec<Rule>, MarshalError>;
    /// Odrzucenie propozycji.
    fn reject(&self, proposal: u64) -> Result<(), MarshalError>;
    /// Cofnięcie aktywnej reguły — wyłącznie użytkownik.
    fn revoke(&self, rule: &RuleId, approver: Approver) -> Result<Rule, MarshalError>;
    /// Reguły aktywne.
    fn rules(&self) -> Vec<Rule>;
    /// Polityka efektywna (sufit ∩ reguły).
    fn effective(&self) -> EffectivePolicy;
    /// Nowy sufit (zmiana uprawnień przez użytkownika/Broker).
    fn set_ceiling(&self, ceiling: Ceiling);
    /// Nadzór: zdarzenie magistrali.
    fn observe(&self, event: &Event) -> Vec<Escalation>;
    /// Nadzór: upływ czasu (długie blokady).
    fn check(&self) -> Vec<Escalation>;
    /// Raport dnia.
    fn daily_report(&self, day: NaiveDate) -> DailyReport;
    /// Propozycje, najnowsze pierwsze: oczekujące (≤ [`MAX_PENDING_PROPOSALS`]) i ostatnie
    /// rozstrzygnięte (≤ [`MAX_DECIDED_PROPOSALS`]); po restarcie — z trwałej księgi.
    fn proposals(&self) -> Vec<Proposal> {
        Vec::new()
    }
}

struct State {
    book: RuleBook,
    watch: Watch,
}

/// Rdzeń z otoczeniem.
pub struct MarshalCore<H: MarshalHost> {
    host: Arc<H>,
    translator: Arc<dyn RuleTranslator>,
    state: Mutex<State>,
}

impl<H: MarshalHost> MarshalCore<H> {
    /// Nowy rdzeń.
    pub fn new(
        host: Arc<H>,
        translator: Arc<dyn RuleTranslator>,
        book: RuleBook,
        watch: Watch,
    ) -> Self {
        Self {
            host,
            translator,
            state: Mutex::new(State { book, watch }),
        }
    }

    /// Otoczenie.
    pub fn host(&self) -> &Arc<H> {
        &self.host
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn change<T>(&self, f: impl FnOnce(&mut RuleBook, u64) -> (T, Vec<Event>)) -> T {
        let now = self.host.now_ms();
        let (value, events, book) = {
            let mut st = self.lock();
            let (value, mut events) = f(&mut st.book, now);
            for id in st
                .book
                .prune_proposals(MAX_PENDING_PROPOSALS, MAX_DECIDED_PROPOSALS)
            {
                events.push(marshal_event(
                    EVENT_REJECTED,
                    now,
                    json!({ "proposal": id, "reason": "limit" }),
                ));
            }
            (value, events, st.book.clone())
        };
        if !events.is_empty() {
            self.host.emit(events);
        }
        self.host.persist(&book);
        value
    }

    /// Księga reguł (zapis, diagnostyka).
    pub fn book(&self) -> RuleBook {
        self.lock().book.clone()
    }

    /// Raport dnia jako zdarzenie (wysyłany przez sterownik o stałej porze).
    pub fn publish_report(&self, day: NaiveDate) -> DailyReport {
        let report = self.daily_report(day);
        let payload = serde_json::to_value(&report).unwrap_or(Value::Null);
        self.host.emit(vec![marshal_event(
            EVENT_REPORT,
            self.host.now_ms(),
            payload,
        )]);
        report
    }
}

fn proposal_event(p: &Proposal, at: u64) -> Event {
    marshal_event(
        EVENT_PROPOSED,
        at,
        json!({
            "proposal": p.id,
            "rules": p.rules.iter().map(|r| &r.id).collect::<Vec<_>>(),
            "rejected": p.rejected.len(),
            "conflicts": p.conflicts,
        }),
    )
}

#[async_trait]
impl<H: MarshalHost> Marshal for MarshalCore<H> {
    async fn propose(&self, text: &str) -> Result<Proposal, MarshalError> {
        let ceiling = self.lock().book.ceiling().clone();
        let drafts = self
            .translator
            .translate(text, &ceiling)
            .await
            .map_err(MarshalError::Translator)?;
        Ok(self.propose_drafts(text, drafts))
    }

    fn propose_drafts(&self, text: &str, drafts: Vec<Value>) -> Proposal {
        self.change(|book, now| {
            let p = book.propose(text, drafts, now);
            let ev = proposal_event(&p, now);
            (p, vec![ev])
        })
    }

    fn approve(&self, proposal: u64, approver: Approver) -> Result<Vec<Rule>, MarshalError> {
        self.change(|book, now| match book.approve(proposal, &approver) {
            Ok(rules) => {
                let ev = marshal_event(
                    EVENT_APPROVED,
                    now,
                    json!({ "proposal": proposal, "rules": rules.iter().map(|r| &r.id).collect::<Vec<_>>(), "approver": approver }),
                );
                (Ok(rules), vec![ev])
            }
            Err(e) => (Err(e), Vec::new()),
        })
    }

    fn reject(&self, proposal: u64) -> Result<(), MarshalError> {
        self.change(|book, now| match book.reject(proposal) {
            Ok(()) => (
                Ok(()),
                vec![marshal_event(
                    EVENT_REJECTED,
                    now,
                    json!({ "proposal": proposal }),
                )],
            ),
            Err(e) => (Err(e), Vec::new()),
        })
    }

    fn revoke(&self, rule: &RuleId, approver: Approver) -> Result<Rule, MarshalError> {
        self.change(|book, now| match book.revoke(rule, &approver) {
            Ok(r) => (
                Ok(r),
                vec![marshal_event(
                    EVENT_REVOKED,
                    now,
                    json!({ "rule": rule, "approver": approver }),
                )],
            ),
            Err(e) => (Err(e), Vec::new()),
        })
    }

    fn rules(&self) -> Vec<Rule> {
        self.lock().book.active()
    }

    fn effective(&self) -> EffectivePolicy {
        self.lock().book.effective()
    }

    fn set_ceiling(&self, ceiling: Ceiling) {
        self.change(|book, _| {
            book.set_ceiling(ceiling);
            ((), Vec::new())
        });
    }

    fn observe(&self, event: &Event) -> Vec<Escalation> {
        let found = self.lock().watch.on_event(event);
        if !found.is_empty() {
            self.host.emit(found.iter().map(escalation_event).collect());
        }
        found
    }

    fn check(&self) -> Vec<Escalation> {
        let now = self.host.now_ms();
        let found = self.lock().watch.check(now);
        if !found.is_empty() {
            self.host.emit(found.iter().map(escalation_event).collect());
        }
        found
    }

    fn daily_report(&self, day: NaiveDate) -> DailyReport {
        self.lock().watch.report(day)
    }

    fn proposals(&self) -> Vec<Proposal> {
        self.lock().book.proposals()
    }
}
