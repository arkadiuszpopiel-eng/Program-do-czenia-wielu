//! Symulacja wykonawczyń na wirtualnym zegarze: każde wysłanie to [`ScriptRun`] z czasem
//! ukończenia bieżącego kroku. Zdarzenia przetwarzane w kolejności (czas, wysłanie).

use std::collections::BTreeMap;

use scheduler_contract::contract_tests::{Script, ScriptRun};
use scheduler_contract::{Dispatch, DispatchId, SchedEffect, SteerEnvelope, TaskId};

/// Wykonawczyni w toku.
pub(crate) struct Worker {
    pub(crate) task: TaskId,
    pub(crate) run: ScriptRun,
    /// Kiedy kończy się bieżący krok (`None` = zawieszona).
    pub(crate) next_at: Option<u64>,
    /// Ile wpisów steeringu już przepisano do księgi.
    pub(crate) recorded: usize,
}

/// Stan symulacji.
#[derive(Default)]
pub(crate) struct Sim {
    pub(crate) scripts: BTreeMap<TaskId, Script>,
    pub(crate) workers: BTreeMap<DispatchId, Worker>,
    pub(crate) seen: BTreeMap<TaskId, Vec<(u32, SteerEnvelope)>>,
    pub(crate) dispatches: Vec<Dispatch>,
    pub(crate) aborted: Vec<DispatchId>,
}

impl Sim {
    /// Wykonuje efekty rdzenia: start wykonawczyń, przerwania.
    pub(crate) fn apply(&mut self, effects: Vec<SchedEffect>, now: u64) {
        for effect in effects {
            match effect {
                SchedEffect::Dispatch(d) => self.start(*d, now),
                SchedEffect::Abort { dispatch, .. } => {
                    self.workers.remove(&dispatch);
                    self.aborted.push(dispatch);
                }
                SchedEffect::Finished { .. } => {}
            }
        }
    }

    fn start(&mut self, dispatch: Dispatch, now: u64) {
        let script = self
            .scripts
            .get(&dispatch.task)
            .cloned()
            .unwrap_or_default();
        let run = ScriptRun::new(script, &dispatch);
        let mut worker = Worker {
            task: dispatch.task.clone(),
            next_at: run.step_duration().map(|ms| now.saturating_add(ms)),
            run,
            recorded: 0,
        };
        self.record(&mut worker);
        self.workers.insert(dispatch.dispatch, worker);
        self.dispatches.push(dispatch);
    }

    /// Przepisuje nowy steering wykonawczyni do księgi.
    pub(crate) fn record(&mut self, worker: &mut Worker) {
        let seen = worker.run.seen();
        let fresh = &seen[worker.recorded.min(seen.len())..];
        self.seen
            .entry(worker.task.clone())
            .or_default()
            .extend(fresh.iter().cloned());
        worker.recorded = seen.len();
    }

    /// Najbliższe ukończenie kroku.
    pub(crate) fn next_step_at(&self) -> Option<u64> {
        self.workers.values().filter_map(|w| w.next_at).min()
    }

    /// Wysłania, których krok kończy się najpóźniej w `now` (kolejność wysłań).
    pub(crate) fn due(&self, now: u64) -> Vec<DispatchId> {
        self.workers
            .iter()
            .filter(|(_, w)| w.next_at.is_some_and(|t| t <= now))
            .map(|(d, _)| *d)
            .collect()
    }
}
