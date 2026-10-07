//! Kontrakt `agent-runtime` (docs/modules/agent-runtime/SPEC.md, PLAN §9.1, §9.6, §16.2 F3).
//!
//! v0 (F3): jedna agentka, pętla **plan → akcja (wywołanie narzędzia) → obserwacja →
//! weryfikacja**, narzędzia sekwencyjnie, bez DAG. Zawiera: specyfikację przebiegu z budżetami
//! ([`RunSpec`], [`RunBudget`]), zdarzenia `agent.*` dla UI/Replay ([`RunEvent`]) z odtwarzaniem
//! stanu z dziennika ([`RunStatus::replay`]), sterowanie w locie ([`Steer`]), checkpointy
//! ([`Checkpoint`], [`CheckpointStore`]) oraz trait [`AgentRuntime`]. Wyniki narzędzi są
//! treścią niezaufaną — runtime delimituje je w prompcie i oznacza sesję (taint).
//!
//! v1 (F5, addytywnie): opcje przebiegu ([`RunOptions`]: obsada do delegacji i Krytyczki,
//! koperta uprawnień [`RunGrant`] z atenuacją „potomek ≤ rodzic”, taint i proweniencja
//! odziedziczone), raport końcowy ([`RunReport`]) i podprzebiegi ([`AgentRuntime::children`]).
//! Zdarzenia `agent.*` bez nowych wariantów: delegacja to krok narzędzia [`DELEGATE_TOOL`],
//! Krytyczka to krok `Verify` + `Verified`, oddanie zadania schedulerowi to `Paused`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod checkpoint;
mod event;
mod grant;
mod options;
mod report;
mod spec;
mod taint;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
/// Współdzielone testy kontraktowe v1 (opcje, raport, równoległość) — `-impl` i `-fake`.
#[cfg(feature = "contract-tests")]
pub mod contract_tests_v1;

pub use checkpoint::{
    CHECKPOINT_VERSION, Checkpoint, CheckpointError, CheckpointStore, MemCheckpointStore,
};
pub use event::{
    RunEvent, RunEventEnvelope, RunOutcome, RunStatus, StepKind, StepStatus, UsageTotals,
};
pub use grant::{RunGrant, budget_within, min_budget};
pub use options::{
    AgentTaskPayload, Crew, DELEGATE_GROUP, DELEGATE_TOOL, DelegateArgs, RunOptions,
};
pub use report::{RunReport, StepLine, VerificationLine, steps_before_delivery};
pub use spec::{BudgetKind, RunBudget, RunSpec};
// Przegląd #2, P2-07: skażenie na poziomie sesji (monotoniczne, reset tylko przez właściciela).
pub use taint::{MemorySessionTaint, SessionTaint, TaintReset, TaintResetError};

pub use core_bus_contract::RunId;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Sterowanie w locie (PLAN §9.6) — przyjmowane między atomowymi krokami.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "steer", content = "value", rename_all = "snake_case")]
pub enum Steer {
    /// Wiadomość/korekta właściciela (uwzględniona w następnym kroku).
    Message(String),
    /// Pauza po bieżącym kroku.
    PauseAfterCurrent,
    /// Wznowienie po pauzie.
    Resume,
    /// Zmiana celu.
    ChangeGoal(String),
    /// Anulowanie.
    Cancel,
}

/// Błędy runtime.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    /// Nieznany przebieg.
    #[error("nieznany przebieg {0}")]
    UnknownRun(RunId),
    /// Specyfikacja niepoprawna.
    #[error("niepoprawna specyfikacja przebiegu: {0}")]
    InvalidSpec(String),
    /// Przebieg już zakończony.
    #[error("przebieg {0} już się zakończył")]
    AlreadyFinished(RunId),
    /// Przebieg już działa (wznowienie).
    #[error("przebieg {0} już działa")]
    AlreadyRunning(RunId),
    /// Brak checkpointu do wznowienia.
    #[error("brak checkpointu przebiegu {0}")]
    NoCheckpoint(RunId),
    /// Magazyn checkpointów.
    #[error("{0}")]
    Store(String),
}

/// Runtime agentki.
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    /// Startuje przebieg (pętla w tle); zwraca identyfikator.
    async fn start(&self, spec: RunSpec) -> Result<RunId, RunError>;

    /// Sterowanie (przyjmowane w najbliższym punkcie atomowym, ≤ 1 krok).
    fn steer(&self, run: &RunId, steer: Steer) -> Result<(), RunError>;

    /// Anulowanie (zatrzymanie w punkcie atomowym, ≤ 2 s; narzędzia i model dostają sygnał).
    fn cancel(&self, run: &RunId) -> Result<(), RunError>;

    /// Wznowienie przebiegu z ostatniego checkpointu (po restarcie).
    async fn resume(&self, run: &RunId) -> Result<(), RunError>;

    /// Czeka na koniec przebiegu.
    async fn wait(&self, run: &RunId) -> Result<RunOutcome, RunError>;

    /// Stan przebiegu.
    fn status(&self, run: &RunId) -> Result<RunStatus, RunError>;

    /// Dziennik zdarzeń przebiegu (Replay).
    fn events(&self, run: &RunId) -> Result<Vec<RunEventEnvelope>, RunError>;

    /// Subskrypcja zdarzeń przebiegu (UI na żywo).
    fn subscribe(
        &self,
        run: &RunId,
    ) -> Result<tokio::sync::broadcast::Receiver<RunEventEnvelope>, RunError>;

    /// v1: start z opcjami (delegacja, Krytyczka, koperta uprawnień, taint odziedziczony).
    /// Domyślnie: opcje domyślne = [`AgentRuntime::start`], inne — `InvalidSpec` (runtime v0).
    async fn start_with(&self, spec: RunSpec, options: RunOptions) -> Result<RunId, RunError> {
        if options == RunOptions::default() {
            self.start(spec).await
        } else {
            Err(RunError::InvalidSpec(
                "ten runtime nie obsługuje opcji v1 (delegacja, Krytyczka, koperta)".into(),
            ))
        }
    }

    /// v1: podprzebiegi (delegacje, Krytyczka) w kolejności startu.
    fn children(&self, run: &RunId) -> Result<Vec<RunId>, RunError> {
        self.events(run).map(|_| Vec::new())
    }

    /// v1: raport końcowy (co zrobiono, kroki cofalne, koszty) z raportami podprzebiegów.
    fn report(&self, run: &RunId) -> Result<RunReport, RunError> {
        let mut report = RunReport::from_events(run.clone(), &self.events(run)?);
        for child in self.children(run)? {
            report.children.push(self.report(&child)?);
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use safety_broker_contract::ApprovalId;

    fn finished(outcome: RunOutcome) -> RunEvent {
        RunEvent::Finished { outcome }
    }

    #[test]
    fn replay_and_names() {
        let events = [
            RunEvent::StepStarted {
                step: 1,
                kind: StepKind::Plan,
                tool: None,
                input: String::new(),
            },
            RunEvent::WaitingApproval {
                step: 1,
                approval: ApprovalId(3),
                explanation: String::new(),
            },
            RunEvent::Paused,
            RunEvent::Resumed,
            finished(RunOutcome::Cancelled),
            RunEvent::Paused,
        ];
        assert_eq!(
            RunStatus::replay(&events[..2]),
            RunStatus::WaitingApproval {
                step: 1,
                approval: ApprovalId(3)
            }
        );
        assert_eq!(
            RunStatus::replay(&events[..3]),
            RunStatus::Paused { step: 1 }
        );
        assert_eq!(
            RunStatus::replay(&events),
            RunStatus::Finished {
                outcome: RunOutcome::Cancelled
            }
        );
        assert!(RunStatus::replay(&events).is_finished());
        assert_eq!(events[5].name(), "agent.run.paused");
        let env = RunEventEnvelope {
            run: RunId::new("r"),
            seq: 1,
            at_ms: 0,
            event: RunEvent::LoopDetected {
                tool: "t".into(),
                repeats: 3,
            },
        };
        let bus = env.to_bus_event(&"s".into(), &"delta".into());
        assert_eq!(bus.kind.as_str(), "agent.run.loop_detected");
        assert_eq!(bus.level, core_bus_contract::Level::Warn);
        assert_eq!(bus.payload["event"]["type"], "loop_detected");
        assert_eq!(BudgetKind::Wall.label_pl(), "czasu");
        assert_eq!(
            UsageTotals {
                input_tokens: 2,
                output_tokens: 3,
                ..Default::default()
            }
            .tokens(),
            5
        );
    }

    #[test]
    fn steer_serde() {
        let s = Steer::Message("dodaj PDF-y".into());
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"steer": "message", "value": "dodaj PDF-y"})
        );
        assert_eq!(serde_json::from_value::<Steer>(json).unwrap(), s);
        assert!(
            RunError::UnknownRun(RunId::new("x"))
                .to_string()
                .contains("x")
        );
    }

    fn event() -> impl Strategy<Value = RunEvent> {
        prop_oneof![
            (1u32..50).prop_map(|step| RunEvent::StepStarted {
                step,
                kind: StepKind::Tool,
                tool: None,
                input: String::new()
            }),
            (1u32..50, 1u64..9).prop_map(|(step, a)| RunEvent::WaitingApproval {
                step,
                approval: ApprovalId(a),
                explanation: String::new()
            }),
            Just(RunEvent::Paused),
            Just(RunEvent::Resumed),
            Just(RunEvent::Steered {
                message: "x".into()
            }),
            Just(finished(RunOutcome::Cancelled)),
        ]
    }

    proptest! {
        /// Replay jest przyrostowy (stan po n+1 = apply(stan po n)) i koniec jest pochłaniający.
        #[test]
        fn replay_is_incremental_and_finish_absorbs(events in proptest::collection::vec(event(), 0..40)) {
            let mut s = RunStatus::Running { step: 0 };
            for (i, e) in events.iter().enumerate() {
                s = s.apply(e);
                prop_assert_eq!(&s, &RunStatus::replay(&events[..=i]));
            }
            if let Some(pos) = events.iter().position(|e| matches!(e, RunEvent::Finished { .. })) {
                prop_assert_eq!(RunStatus::replay(&events), RunStatus::replay(&events[..=pos]));
            }
        }
    }
}
