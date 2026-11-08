//! Atrapa Broker-UI (docs/modules/broker-ui/SPEC.md, sekcja „Fake”): skryptowane decyzje
//! z syntetycznym `PhysicalInputProof` (nonce z wyzwania) — tylko do testów modułów, które
//! potrzebują „właściciela klikającego w oknie Brokera”.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::{BTreeMap, VecDeque};

use broker_ui_contract::{
    BrokerUi, GRANT_MAX_MS, RejectReason, UiDecision, UiError, UiEvent, UiStatus,
};
use safety_broker_contract::{
    ApprovalChallenge, ApprovalDecision, ApprovalId, ApprovalSubject, InputSource, broker_ui_only,
};

/// Skryptowana reakcja „właściciela”.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    /// „Tylko teraz”.
    Allow,
    /// „Zawsze w tym zakresie” przez `hours` (przycinane do 24 h; tylko pojedyncza akcja).
    AllowInScope {
        /// Godziny.
        hours: u8,
    },
    /// „Odmów”.
    Deny,
    /// Próba wstrzyknięcia (SendInput) — odrzucona, bez decyzji.
    Injected,
    /// Brak reakcji (karta czeka).
    Ignore,
}

/// Skryptowany Broker-UI.
#[derive(Debug, Default)]
pub struct ScriptedBrokerUi {
    queue: VecDeque<ApprovalChallenge>,
    per_id: BTreeMap<ApprovalId, Script>,
    fifo: VecDeque<Script>,
    default: Option<Script>,
    events: Vec<UiEvent>,
}

impl ScriptedBrokerUi {
    /// Atrapa bez skryptu (karty czekają).
    pub fn new() -> Self {
        Self::default()
    }

    /// Reakcja dla konkretnej prośby.
    pub fn script_for(&mut self, id: ApprovalId, script: Script) {
        self.per_id.insert(id, script);
    }

    /// Reakcje dla kolejnych kart (gdy brak skryptu per prośba).
    pub fn push_script(&mut self, script: Script) {
        self.fifo.push_back(script);
    }

    /// Reakcja domyślna, gdy skrypt się wyczerpie.
    pub fn set_default(&mut self, script: Option<Script>) {
        self.default = script;
    }

    fn next_script(&mut self, id: ApprovalId) -> Script {
        self.per_id
            .remove(&id)
            .or_else(|| self.fifo.pop_front())
            .or(self.default)
            .unwrap_or(Script::Ignore)
    }
}

fn decision_of(ch: &ApprovalChallenge, script: Script, now_ms: u64) -> Option<ApprovalDecision> {
    match script {
        Script::Allow => Some(ApprovalDecision::Allow),
        Script::Deny => Some(ApprovalDecision::Deny),
        Script::AllowInScope { hours } => match &ch.request.subject {
            ApprovalSubject::Action { capability, .. } if ch.request.grantable => {
                let ms = (u64::from(hours.max(1)) * 3_600_000).min(GRANT_MAX_MS);
                Some(ApprovalDecision::AllowInScope {
                    scope: capability.clone(),
                    until_ms: now_ms.saturating_add(ms),
                })
            }
            _ => Some(ApprovalDecision::Allow),
        },
        Script::Injected | Script::Ignore => None,
    }
}

impl BrokerUi for ScriptedBrokerUi {
    fn show(&mut self, challenge: ApprovalChallenge, now_ms: u64) -> Result<(), UiError> {
        if challenge.request.expires_at_ms <= now_ms {
            return Err(UiError::InvalidChallenge("prośba już wygasła".into()));
        }
        if self
            .queue
            .iter()
            .all(|c| c.request.id != challenge.request.id)
        {
            self.events.push(UiEvent::Shown {
                id: challenge.request.id,
            });
            self.queue.push_back(challenge);
        }
        Ok(())
    }

    fn poll_decision(&mut self, now_ms: u64, _wait_ms: u32) -> Option<UiDecision> {
        let front = self.queue.front()?.clone();
        let id = front.request.id;
        let script = self.next_script(id);
        if script == Script::Injected {
            self.events.push(UiEvent::InputRejected {
                id,
                reason: RejectReason::Injected,
            });
            return None;
        }
        let decision = decision_of(&front, script, now_ms)?;
        let at_ms = now_ms.clamp(
            front.request.created_at_ms,
            front.request.expires_at_ms.saturating_sub(1),
        );
        let proof = broker_ui_only::physical_input_proof(
            id,
            front.nonce,
            InputSource::MouseClick,
            false,
            at_ms,
        );
        self.queue.pop_front();
        self.events.push(UiEvent::Decided {
            id,
            decision: decision.clone(),
        });
        Some(UiDecision {
            id,
            decision,
            proof,
        })
    }

    fn withdraw(&mut self, id: ApprovalId) {
        let before = self.queue.len();
        self.queue.retain(|c| c.request.id != id);
        if self.queue.len() != before {
            self.events.push(UiEvent::Withdrawn { id });
        }
    }

    fn queued(&self) -> Vec<ApprovalId> {
        self.queue.iter().map(|c| c.request.id).collect()
    }

    fn status(&self) -> UiStatus {
        match self.queue.len() {
            0 => UiStatus::Hidden,
            n => UiStatus::Pending {
                count: u8::try_from(n).unwrap_or(u8::MAX),
            },
        }
    }

    fn drain_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.events)
    }
}
