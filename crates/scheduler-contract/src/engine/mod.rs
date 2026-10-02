//! Deterministyczny rdzeń decyzyjny schedulera ([`SchedCore`]): stan zadań (serializowalny),
//! dzierżawy zasobów z warstwy `scheduler-lite` (jedna tablica blokad z mową) i operacje
//! zwracające efekty dla sterownika ([`SchedEffect`]). Ta sama sekwencja wywołań z tymi samymi
//! czasami daje te same decyzje — `-impl` i `-fake` różnią się tylko otoczeniem ([`SchedHost`]).
//!
//! Gwarancje (testy własności w `scheduler-fake`):
//! - zadanie dostaje komplet zasobów atomowo albo nic (brak „hold and wait”) i nigdy nie czeka,
//!   trzymając zasób → brak cykli oczekiwania między zadaniami;
//! - graf zależności i delegacji jest acykliczny (walidacja przy zgłoszeniu);
//! - każde zadanie ma skończony termin, budżet czasu i kroków oraz skończoną liczbę prób → kończy
//!   się w skończonym czasie z jawnym powodem ([`Termination`]).

mod control;
mod dispatch;
mod graph;
mod persist;
mod run;
mod time;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::Event;
use personas_contract::PersonaId;
use scheduler_lite_contract::{Core, Lease};
use serde::{Deserialize, Serialize};

use crate::host::{LiteBridge, SchedHost};
use crate::ids::{DispatchId, TaskId};
use crate::roster::{Roster, SystemConditions};
use crate::spec::TaskSpec;
use crate::state::{BlockReason, TaskState, Termination};
use crate::steer::{Dispatch, SteerEnvelope, StopReason, YieldReason};

pub use persist::{SNAPSHOT_VERSION, Snapshot};

/// Efekt decyzji do wykonania przez sterownik (poza blokadą rdzenia, w tej kolejności).
#[derive(Debug, Clone, PartialEq)]
pub enum SchedEffect {
    /// Uruchom wykonawczynię.
    Dispatch(Box<Dispatch>),
    /// Przerwij wykonawczynię siłą (brak punktu atomowego w czasie, kill-switch).
    Abort {
        /// Zadanie.
        task: TaskId,
        /// Wysłanie.
        dispatch: DispatchId,
    },
    /// Zadanie zakończone (dla czekających na `wait`).
    Finished {
        /// Zadanie.
        task: TaskId,
        /// Zakończenie.
        termination: Termination,
    },
}

/// Rekord zadania (serializowalny — trwałość).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TaskRec {
    pub(crate) spec: TaskSpec,
    pub(crate) seq: u64,
    pub(crate) submitted_at_ms: u64,
    pub(crate) deadline_ms: u64,
    pub(crate) state: TaskState,
    pub(crate) failures: u32,
    pub(crate) steps: u32,
    pub(crate) cost_micro_pln: u64,
    pub(crate) wall_ms: u64,
    pub(crate) preemptions: u32,
    pub(crate) blocked: Option<BlockReason>,
    pub(crate) steering: Vec<SteerEnvelope>,
    pub(crate) children: Vec<TaskId>,
    pub(crate) yield_request: Option<YieldReason>,
    pub(crate) stop_request: Option<StopReason>,
    pub(crate) stop_requested_at_ms: Option<u64>,
    pub(crate) last_fingerprint: Option<u64>,
    pub(crate) repeats: u32,
    pub(crate) last_dispatch: Option<DispatchId>,
    pub(crate) finished_at_ms: Option<u64>,
    pub(crate) interrupted: bool,
    /// Pierwszy start wykonania (zostaje po zakończeniu; stan sprzed tej wersji: brak).
    #[serde(default)]
    pub(crate) first_started_at_ms: Option<u64>,
    /// Agentka ostatniego wysłania (zostaje po zakończeniu).
    #[serde(default)]
    pub(crate) last_agent: Option<PersonaId>,
}

impl TaskRec {
    pub(crate) fn new(spec: TaskSpec, seq: u64, now_ms: u64, deadline_ms: u64) -> Self {
        Self {
            spec,
            seq,
            submitted_at_ms: now_ms,
            deadline_ms,
            state: TaskState::Pending,
            failures: 0,
            steps: 0,
            cost_micro_pln: 0,
            wall_ms: 0,
            preemptions: 0,
            blocked: Some(BlockReason::Dependencies),
            steering: Vec::new(),
            children: Vec::new(),
            yield_request: None,
            stop_request: None,
            stop_requested_at_ms: None,
            last_fingerprint: None,
            repeats: 0,
            last_dispatch: None,
            finished_at_ms: None,
            interrupted: false,
            first_started_at_ms: None,
            last_agent: None,
        }
    }

    pub(crate) fn running_dispatch(&self) -> Option<DispatchId> {
        match self.state {
            TaskState::Running { dispatch, .. } => Some(dispatch),
            _ => None,
        }
    }

    pub(crate) fn agent_label(&self) -> Option<String> {
        match &self.state {
            TaskState::Running {
                agent: Some(agent), ..
            } => Some(agent.to_string()),
            _ => None,
        }
    }
}

/// Stan rdzenia (serializowalny).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct State {
    pub(crate) tasks: BTreeMap<TaskId, TaskRec>,
    pub(crate) seq: u64,
    pub(crate) next_dispatch: u64,
    pub(crate) next_steer: u64,
    pub(crate) roster: Roster,
    pub(crate) conditions: SystemConditions,
    /// Numer zmiany stanu (rośnie przy każdej zmianie) — magazyn nie nadpisuje nowszego starszym.
    #[serde(default)]
    pub(crate) revision: u64,
}

pub(crate) struct Inner {
    pub(crate) st: State,
    /// Dzierżawy zadań w toku (nie są utrwalane — po restarcie zasoby są wolne).
    pub(crate) leases: BTreeMap<TaskId, Vec<Lease>>,
}

/// Wyniki operacji: efekty dla sterownika, zdarzenia, czy stan się zmienił.
#[derive(Default)]
pub(crate) struct Out {
    pub(crate) effects: Vec<SchedEffect>,
    pub(crate) events: Vec<Event>,
    pub(crate) dirty: bool,
}

/// Kontekst operacji (pod blokadą rdzenia).
pub(crate) struct Ctx<'a, H: SchedHost> {
    pub(crate) locks: &'a Core<LiteBridge<H>>,
    pub(crate) host: &'a H,
    pub(crate) inner: &'a mut Inner,
    pub(crate) now: u64,
    pub(crate) out: &'a mut Out,
}

/// Rdzeń pełnego schedulera.
pub struct SchedCore<H: SchedHost> {
    host: Arc<H>,
    locks: Arc<Core<LiteBridge<H>>>,
    inner: Mutex<Inner>,
}

impl<H: SchedHost> SchedCore<H> {
    /// Nowy, pusty rdzeń.
    pub fn new(host: Arc<H>) -> Arc<Self> {
        Self::with_state(host, State::default())
    }

    pub(crate) fn with_state(host: Arc<H>, st: State) -> Arc<Self> {
        let locks = Core::new(LiteBridge(Arc::clone(&host)));
        Arc::new(Self {
            host,
            locks,
            inner: Mutex::new(Inner {
                st,
                leases: BTreeMap::new(),
            }),
        })
    }

    /// Otoczenie.
    pub fn host(&self) -> &Arc<H> {
        &self.host
    }

    /// Warstwa zasobów (`scheduler-lite`) — ta sama tablica blokad dla mowy i zadań.
    pub fn locks(&self) -> &Arc<Core<LiteBridge<H>>> {
        &self.locks
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Wykonuje operację pod blokadą, potem przegląd kolejki (`pump`); zdarzenia i zapis stanu
    /// po zwolnieniu blokady. Błąd = brak zmian.
    pub(crate) fn op<T>(
        &self,
        f: impl FnOnce(&mut Ctx<'_, H>) -> Result<T, crate::error::TaskError>,
    ) -> Result<(T, Vec<SchedEffect>), crate::error::TaskError> {
        self.op_with(true, f)
    }

    /// Jak [`Self::op`]; `pump = false` — bez przeglądu kolejki (np. odtworzenie stanu: efekty
    /// przydziału odbierze dopiero sterownik przez `pump`).
    pub(crate) fn op_with<T>(
        &self,
        pump: bool,
        f: impl FnOnce(&mut Ctx<'_, H>) -> Result<T, crate::error::TaskError>,
    ) -> Result<(T, Vec<SchedEffect>), crate::error::TaskError> {
        let now = self.host.now_ms();
        let mut out = Out::default();
        let (value, snapshot) = {
            let mut guard = self.lock();
            let mut ctx = Ctx {
                locks: &self.locks,
                host: &self.host,
                inner: &mut guard,
                now,
                out: &mut out,
            };
            let value = f(&mut ctx)?;
            if pump {
                ctx.pump();
            }
            let snapshot = out.dirty.then(|| {
                guard.st.revision += 1;
                guard.st.snapshot(now)
            });
            (value, snapshot)
        };
        if !out.events.is_empty() {
            self.host.emit(std::mem::take(&mut out.events));
        }
        if let Some(snapshot) = snapshot {
            self.host.persist(&snapshot);
            // Stan się zmienił → najbliższy termin mógł się przesunąć (sterownik przelicza timer).
            self.host.wake();
        }
        Ok((value, out.effects))
    }

    /// Odczyt stanu pod blokadą.
    pub(crate) fn read<T>(&self, f: impl FnOnce(&Inner) -> T) -> T {
        f(&self.lock())
    }
}
