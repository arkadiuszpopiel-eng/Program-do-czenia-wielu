//! Deterministyczny rdzeń wyzwalaczy ([`TriggerEngine`]): rejestr, terminy czasowe, zarządzanie
//! (właściciel), stan serializowalny. Wyzwalanie i budowa zadań — `fire.rs`.

mod fire;
mod input;

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::record::{FireCause, RunRecord, TriggerView};
use crate::spec::{Actor, TriggerId, TriggerKind, TriggerSpec};
use crate::validate::{MAX_TRIGGERS, TriggerError, may_manage, validate};

pub use fire::{TaskSink, task_for};

/// Wersja formatu stanu.
pub const TRIGGERS_SNAPSHOT_VERSION: u32 = 1;

/// Odłożone uruchomienie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Deferred {
    pub(crate) until_ms: u64,
    pub(crate) cause: FireCause,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TrigRec {
    pub(crate) spec: TriggerSpec,
    pub(crate) created_at_ms: u64,
    pub(crate) next_fire_ms: Option<u64>,
    pub(crate) last_fire_ms: Option<u64>,
    pub(crate) recent: Vec<u64>,
    pub(crate) fired: u64,
    pub(crate) suppressed: u64,
    pub(crate) seq: u64,
    pub(crate) deferred: Option<Deferred>,
    pub(crate) log: VecDeque<RunRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct State {
    pub(crate) triggers: BTreeMap<TriggerId, TrigRec>,
    pub(crate) global_recent: Vec<u64>,
    pub(crate) log: VecDeque<RunRecord>,
    pub(crate) dnd: bool,
}

/// Utrwalany stan wyzwalaczy (JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerSnapshot {
    /// Wersja formatu.
    pub version: u32,
    state: State,
}

impl TriggerSnapshot {
    /// Liczba wyzwalaczy.
    pub fn len(&self) -> usize {
        self.state.triggers.len()
    }

    /// Czy pusty.
    pub fn is_empty(&self) -> bool {
        self.state.triggers.is_empty()
    }
}

/// Następne wystąpienie czasowe ściśle po `after_ms`.
pub(crate) fn next_time(spec: &TriggerSpec, created_at_ms: u64, after_ms: u64) -> Option<u64> {
    match &spec.kind {
        TriggerKind::Cron { expr } => expr.next_after(after_ms, &spec.tz),
        TriggerKind::Once { at_ms } => (*at_ms > after_ms).then_some(*at_ms),
        TriggerKind::Interval { every_ms, start_ms } => {
            let start = start_ms.unwrap_or(created_at_ms);
            if after_ms < start {
                return Some(start);
            }
            let k = (after_ms - start) / every_ms + 1;
            start.checked_add(k.checked_mul(*every_ms)?)
        }
        _ => None,
    }
}

/// Rdzeń wyzwalaczy.
#[derive(Debug, Clone, Default)]
pub struct TriggerEngine {
    pub(crate) st: State,
}

impl TriggerEngine {
    /// Pusty rejestr.
    pub fn new() -> Self {
        Self::default()
    }

    /// Odtworzenie ze stanu.
    pub fn restore(snapshot: TriggerSnapshot) -> Result<Self, String> {
        if snapshot.version != TRIGGERS_SNAPSHOT_VERSION {
            return Err(format!(
                "nieobsługiwana wersja stanu wyzwalaczy: {}",
                snapshot.version
            ));
        }
        Ok(Self { st: snapshot.state })
    }

    /// Stan do zapisu.
    pub fn snapshot(&self) -> TriggerSnapshot {
        TriggerSnapshot {
            version: TRIGGERS_SNAPSHOT_VERSION,
            state: self.st.clone(),
        }
    }

    /// Dodaje wyzwalacz (właścicielem jest `actor`). Uprawnienia zadania nie są pobierane teraz —
    /// `scope` to tylko sufit; tokeny wydaje Broker przy wykonaniu.
    pub fn add(
        &mut self,
        spec: TriggerSpec,
        actor: &Actor,
        now: u64,
    ) -> Result<TriggerView, TriggerError> {
        validate(&spec, actor, now)?;
        fire::probe(&spec, now)?;
        if self.st.triggers.contains_key(&spec.id) {
            return Err(TriggerError::Duplicate(spec.id));
        }
        if self.st.triggers.len() >= MAX_TRIGGERS {
            return Err(TriggerError::Capacity(MAX_TRIGGERS));
        }
        let id = spec.id.clone();
        let rec = TrigRec {
            next_fire_ms: next_time(&spec, now, now),
            spec,
            created_at_ms: now,
            last_fire_ms: None,
            recent: Vec::new(),
            fired: 0,
            suppressed: 0,
            seq: 0,
            deferred: None,
            log: VecDeque::new(),
        };
        self.st.triggers.insert(id.clone(), rec);
        self.view(&id).ok_or(TriggerError::Unknown(id))
    }

    /// Zmienia wyzwalacz (właściciel bez zmian; użytkownik może zmieniać każdy).
    pub fn update(
        &mut self,
        spec: TriggerSpec,
        actor: &Actor,
        now: u64,
    ) -> Result<TriggerView, TriggerError> {
        let rec = self
            .st
            .triggers
            .get(&spec.id)
            .ok_or_else(|| TriggerError::Unknown(spec.id.clone()))?;
        if !may_manage(actor, &rec.spec.owner) || spec.owner != rec.spec.owner {
            return Err(TriggerError::Forbidden(
                "zmiana cudzego wyzwalacza albo właściciela".into(),
            ));
        }
        validate(&spec, &rec.spec.owner, now)?;
        fire::probe(&spec, now)?;
        let id = spec.id.clone();
        if let Some(rec) = self.st.triggers.get_mut(&id) {
            rec.next_fire_ms = next_time(&spec, rec.created_at_ms, now);
            rec.spec = spec;
            rec.deferred = None;
        }
        self.view(&id).ok_or(TriggerError::Unknown(id))
    }

    /// Usuwa wyzwalacz.
    pub fn remove(&mut self, id: &TriggerId, actor: &Actor) -> Result<(), TriggerError> {
        let rec = self
            .st
            .triggers
            .get(id)
            .ok_or_else(|| TriggerError::Unknown(id.clone()))?;
        if !may_manage(actor, &rec.spec.owner) {
            return Err(TriggerError::Forbidden(
                "usuwanie cudzego wyzwalacza".into(),
            ));
        }
        self.st.triggers.remove(id);
        Ok(())
    }

    /// Włącza/wyłącza (po włączeniu termin liczony od teraz — bez nadrabiania).
    pub fn set_enabled(
        &mut self,
        id: &TriggerId,
        enabled: bool,
        actor: &Actor,
        now: u64,
    ) -> Result<(), TriggerError> {
        let rec = self
            .st
            .triggers
            .get_mut(id)
            .ok_or_else(|| TriggerError::Unknown(id.clone()))?;
        if !may_manage(actor, &rec.spec.owner) {
            return Err(TriggerError::Forbidden("zmiana cudzego wyzwalacza".into()));
        }
        if enabled && !rec.spec.enabled {
            rec.next_fire_ms = next_time(&rec.spec, rec.created_at_ms, now);
        }
        rec.spec.enabled = enabled;
        if !enabled {
            rec.deferred = None;
        }
        Ok(())
    }

    /// Globalne „Nie przeszkadzać”.
    pub fn set_dnd(&mut self, on: bool, now: u64) {
        self.st.dnd = on;
        if !on {
            for rec in self.st.triggers.values_mut() {
                if let Some(d) = rec.deferred.as_mut() {
                    d.until_ms = d.until_ms.min(now);
                }
            }
        }
    }

    /// Czy „Nie przeszkadzać” włączone.
    pub fn dnd(&self) -> bool {
        self.st.dnd
    }

    /// Widok wyzwalacza.
    pub fn view(&self, id: &TriggerId) -> Option<TriggerView> {
        self.st.triggers.get(id).map(|r| TriggerView {
            spec: r.spec.clone(),
            next_fire_ms: r.next_fire_ms,
            last_fire_ms: r.last_fire_ms,
            fired: r.fired,
            suppressed: r.suppressed,
            deferred_until_ms: r.deferred.as_ref().map(|d| d.until_ms),
        })
    }

    /// Wszystkie wyzwalacze.
    pub fn list(&self) -> Vec<TriggerView> {
        self.st
            .triggers
            .keys()
            .filter_map(|id| self.view(id))
            .collect()
    }

    /// Dziennik uruchomień (najnowsze na końcu): wyzwalacza albo wszystkich.
    pub fn log(&self, id: Option<&TriggerId>, limit: usize) -> Vec<RunRecord> {
        let all: Vec<&RunRecord> = match id {
            Some(id) => self
                .st
                .triggers
                .get(id)
                .map(|r| r.log.iter().collect())
                .unwrap_or_default(),
            None => self.st.log.iter().collect(),
        };
        let skip = all.len().saturating_sub(limit);
        all.into_iter().skip(skip).cloned().collect()
    }

    /// Najbliższa chwila, w której coś może się wyzwolić (termin albo koniec odłożenia).
    pub fn next_wake(&self) -> Option<u64> {
        self.st
            .triggers
            .values()
            .filter(|r| r.spec.enabled)
            .flat_map(|r| {
                r.next_fire_ms
                    .into_iter()
                    .chain(r.deferred.as_ref().map(|d| d.until_ms))
            })
            .min()
    }

    /// Katalogi do obserwacji (wyzwalacze plikowe włączone).
    pub fn watched_dirs(&self) -> Vec<String> {
        let mut dirs: Vec<String> = self
            .st
            .triggers
            .values()
            .filter(|r| r.spec.enabled)
            .filter_map(|r| match &r.spec.kind {
                TriggerKind::FileInDir { dir, .. } => Some(dir.clone()),
                _ => None,
            })
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    }
}
