//! Atrapa modelu-sędziego (`docs/modules/approval-judge/SPEC.md`, „Fake”): werdykty z kolejki skryptu,
//! po jej wyczerpaniu werdykt domyślny (`Escalate` — jak prawdziwy sędzia bez modelu), rejestr wejść.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use approval_judge_contract::{ApprovalJudge, JudgeInput, JudgeVerdict};

#[derive(Default)]
struct State {
    queue: VecDeque<JudgeVerdict>,
    default: Option<JudgeVerdict>,
    seen: Vec<JudgeInput>,
}

/// Sędzia ze skryptu.
#[derive(Default)]
pub struct ScriptedJudge {
    state: Mutex<State>,
}

impl ScriptedJudge {
    /// Pusta atrapa: każda ocena kończy się `Escalate`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Atrapa z kolejką werdyktów (kolejne oceny biorą je po kolei).
    pub fn with_queue(verdicts: impl IntoIterator<Item = JudgeVerdict>) -> Self {
        let judge = Self::new();
        judge.lock().queue.extend(verdicts);
        judge
    }

    /// Werdykt po wyczerpaniu kolejki.
    pub fn set_default(&self, verdict: JudgeVerdict) {
        self.lock().default = Some(verdict);
    }

    /// Dopisuje werdykt na koniec kolejki.
    pub fn push(&self, verdict: JudgeVerdict) {
        self.lock().queue.push_back(verdict);
    }

    /// Wejścia, które sędzia dostał (od najstarszego).
    pub fn seen(&self) -> Vec<JudgeInput> {
        self.lock().seen.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Zatruty zamek w atrapie: stan nadal spójny (same push/pop), więc bierzemy go mimo to.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[async_trait::async_trait]
impl ApprovalJudge for ScriptedJudge {
    async fn review(&self, input: &JudgeInput) -> JudgeVerdict {
        let mut st = self.lock();
        st.seen.push(input.clone());
        st.queue
            .pop_front()
            .or_else(|| st.default.clone())
            .unwrap_or_else(|| JudgeVerdict::escalate("atrapa: brak werdyktu w skrypcie"))
    }

    fn model_name(&self) -> String {
        "fake:scripted".to_owned()
    }
}
