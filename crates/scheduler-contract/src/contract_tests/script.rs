//! Skryptowana wykonawczyni: ta sama maszyna stanów ([`ScriptRun`]) napędza wirtualny zegar
//! `-fake` i asynchroniczny [`ScriptedExecutor`] (zegar tokio) w `-impl`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;

use crate::{
    Dispatch, SteerEnvelope, StepDirective, StepGate, StepReport, TaskExecutor, TaskId, TaskOutput,
    WorkerResult,
};

/// Wynik końcowy skryptu.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptOutcome {
    /// Sukces z wynikiem.
    Succeed(TaskOutput),
    /// Porażka.
    Fail {
        /// Ponawialna.
        retryable: bool,
    },
}

/// Zachowanie wykonawczyni dla zadania.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    /// Liczba kroków atomowych (≥ 1).
    pub steps: u32,
    /// Czas kroku (ms).
    pub step_ms: u64,
    /// Wynik po ostatnim kroku.
    pub outcome: ScriptOutcome,
    /// Pierwsze `fail_first` prób kończy się ponawialnym błędem po pierwszym kroku.
    pub fail_first: u32,
    /// Krok (1-based), w którym wykonawczyni się zawiesza (bez punktu atomowego).
    pub hang_at_step: Option<u32>,
    /// Koszt kroku (mikro-PLN).
    pub cost_per_step: u64,
    /// Odcisk kroku (ten sam = podejrzenie pętli).
    pub fingerprint: Option<u64>,
}

impl Script {
    /// `steps` kroków po `step_ms`, sukces z podsumowaniem „ok”.
    pub fn ok(steps: u32, step_ms: u64) -> Self {
        Self {
            steps,
            step_ms,
            outcome: ScriptOutcome::Succeed(TaskOutput::text("ok")),
            fail_first: 0,
            hang_at_step: None,
            cost_per_step: 0,
            fingerprint: None,
        }
    }

    /// Wynik końcowy (builder).
    #[must_use]
    pub fn outcome(mut self, outcome: ScriptOutcome) -> Self {
        self.outcome = outcome;
        self
    }
}

impl Default for Script {
    fn default() -> Self {
        Self::ok(1, 10)
    }
}

/// Przebieg skryptu dla jednego wysłania.
#[derive(Debug, Clone)]
pub struct ScriptRun {
    script: Script,
    attempt: u32,
    step: u32,
    seen: Vec<(u32, SteerEnvelope)>,
}

impl ScriptRun {
    /// Start (steering sprzed startu jest „widziany” przed pierwszym krokiem tego wysłania).
    pub fn new(script: Script, dispatch: &Dispatch) -> Self {
        let first = dispatch.resume_from_step + 1;
        Self {
            script,
            attempt: dispatch.attempt,
            step: dispatch.resume_from_step,
            seen: dispatch
                .steering
                .iter()
                .map(|e| (first, e.clone()))
                .collect(),
        }
    }

    /// Czas następnego kroku (`None` = zawieszenie bez końca).
    pub fn step_duration(&self) -> Option<u64> {
        if self.script.hang_at_step == Some(self.step + 1) {
            None
        } else {
            Some(self.script.step_ms)
        }
    }

    /// Krok ukończony: `Ok(raport)` → punkt atomowy; `Err(wynik)` → koniec wykonania.
    pub fn complete_step(&mut self) -> Result<StepReport, WorkerResult> {
        self.step += 1;
        if self.attempt <= self.script.fail_first {
            return Err(WorkerResult::Failed {
                error: format!("skryptowy błąd próby {}", self.attempt),
                retryable: true,
            });
        }
        if self.step >= self.script.steps.max(1) {
            return Err(match &self.script.outcome {
                ScriptOutcome::Succeed(output) => WorkerResult::Succeeded {
                    output: output.clone(),
                },
                ScriptOutcome::Fail { retryable } => WorkerResult::Failed {
                    error: "skryptowa porażka".into(),
                    retryable: *retryable,
                },
            });
        }
        Ok(StepReport {
            cost_micro_pln: self.script.cost_per_step,
            fingerprint: self.script.fingerprint,
        })
    }

    /// Dyrektywa z punktu atomowego: `None` = następny krok, `Some(wynik)` = koniec.
    pub fn directive(&mut self, directive: StepDirective) -> Option<WorkerResult> {
        match directive {
            StepDirective::Continue { steering } => {
                let next = self.step + 1;
                self.seen.extend(steering.into_iter().map(|e| (next, e)));
                None
            }
            StepDirective::Yield { .. } => Some(WorkerResult::Yielded),
            StepDirective::Stop { .. } => Some(WorkerResult::Stopped),
        }
    }

    /// Ukończone kroki.
    pub fn step(&self) -> u32 {
        self.step
    }

    /// Steering widziany przez wykonawczynię: (krok, przed którym dostarczono, wiadomość).
    pub fn seen(&self) -> &[(u32, SteerEnvelope)] {
        &self.seen
    }
}

#[derive(Default)]
struct Book {
    scripts: BTreeMap<TaskId, Script>,
    seen: BTreeMap<TaskId, Vec<(u32, SteerEnvelope)>>,
    dispatches: Vec<Dispatch>,
}

/// Skryptowana wykonawczyni na zegarze tokio (dla `-impl`; testy z zatrzymanym zegarem).
#[derive(Default, Clone)]
pub struct ScriptedExecutor {
    book: Arc<Mutex<Book>>,
}

impl ScriptedExecutor {
    /// Nowa wykonawczyni.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Book> {
        self.book.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Ustawia skrypt zadania.
    pub fn script(&self, task: &TaskId, script: Script) {
        self.lock().scripts.insert(task.clone(), script);
    }

    /// Steering widziany przez wykonawczynię.
    pub fn seen(&self, task: &TaskId) -> Vec<(u32, SteerEnvelope)> {
        self.lock().seen.get(task).cloned().unwrap_or_default()
    }

    /// Wszystkie wysłania (w kolejności).
    pub fn dispatches(&self) -> Vec<Dispatch> {
        self.lock().dispatches.clone()
    }

    fn record(&self, task: &TaskId, run: &ScriptRun, from: usize) -> usize {
        let fresh = &run.seen()[from.min(run.seen().len())..];
        self.lock()
            .seen
            .entry(task.clone())
            .or_default()
            .extend(fresh.iter().cloned());
        run.seen().len()
    }
}

#[async_trait]
impl TaskExecutor for ScriptedExecutor {
    async fn execute(&self, dispatch: Dispatch, gate: Arc<dyn StepGate>) -> WorkerResult {
        let script = {
            let mut book = self.lock();
            book.dispatches.push(dispatch.clone());
            book.scripts
                .get(&dispatch.task)
                .cloned()
                .unwrap_or_default()
        };
        let task = dispatch.task.clone();
        let mut run = ScriptRun::new(script, &dispatch);
        let mut recorded = self.record(&task, &run, 0);
        loop {
            match run.step_duration() {
                Some(ms) => tokio::time::sleep(std::time::Duration::from_millis(ms)).await,
                None => std::future::pending::<()>().await,
            }
            let report = match run.complete_step() {
                Ok(report) => report,
                Err(result) => return result,
            };
            let directive = gate.boundary(report);
            let end = run.directive(directive);
            recorded = self.record(&task, &run, recorded);
            if let Some(result) = end {
                return result;
            }
        }
    }
}
