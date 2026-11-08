//! Zbiór obserwacji (wspólny dla `ReadDirectoryChangesW` i atrapy): polityka (deny-lista surowa
//! i kanoniczna, limit), identyfikatory, kolejka zdarzeń natychmiastowych (`Rescanned`, `Stopped`)
//! i odbiór zmian po debounce ze wszystkich obserwacji.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use crate::dirwatch::{RescanReason, WatchEvent, WatchId, WatchSpec};
use crate::dirwatch_core::{FileStamp, RawChange, WatchCore};
use crate::dirwatch_policy::WatchPolicy;
use crate::error::PlatformError;

/// Zbiór obserwacji z polityką i kolejką zdarzeń (wspólny dla Windows i atrapy).
#[derive(Debug, Clone)]
pub struct WatchSet {
    policy: WatchPolicy,
    next: u64,
    cores: BTreeMap<WatchId, WatchCore>,
    queue: VecDeque<WatchEvent>,
}

impl WatchSet {
    /// Pusty zbiór.
    pub fn new(policy: WatchPolicy) -> Self {
        Self {
            policy,
            next: 0,
            cores: BTreeMap::new(),
            queue: VecDeque::new(),
        }
    }

    /// Polityka.
    pub fn policy(&self) -> &WatchPolicy {
        &self.policy
    }

    /// Sprawdza obserwację przed jej założeniem (deny-lista surowa i kanoniczna, limit).
    pub fn check(&self, spec: &WatchSpec, canonical: Option<&Path>) -> Result<(), PlatformError> {
        self.policy.check_spec(spec, canonical)?;
        if self.cores.len() >= self.policy.max_watches {
            return Err(PlatformError::PermissionDenied(format!(
                "limit obserwowanych katalogów ({}) wyczerpany",
                self.policy.max_watches
            )));
        }
        Ok(())
    }

    /// Dodaje obserwację (po `check`) ze stanem początkowym.
    pub fn add(
        &mut self,
        spec: WatchSpec,
        canonical: Option<&Path>,
        listing: Vec<(PathBuf, FileStamp)>,
    ) -> Result<WatchId, PlatformError> {
        self.check(&spec, canonical)?;
        self.next += 1;
        let id = WatchId(self.next);
        let core = WatchCore::new(id, spec, self.policy.clone(), listing);
        self.cores.insert(id, core);
        Ok(id)
    }

    /// Usuwa obserwację (oczekujące zmiany przepadają).
    pub fn remove(&mut self, id: WatchId) -> Result<WatchSpec, PlatformError> {
        self.cores
            .remove(&id)
            .map(|c| c.spec().clone())
            .ok_or_else(|| PlatformError::UnknownResource(format!("obserwacja {}", id.0)))
    }

    /// Obserwacja (odczyt).
    pub fn core(&self, id: WatchId) -> Option<&WatchCore> {
        self.cores.get(&id)
    }

    /// Obserwacja.
    pub fn core_mut(&mut self, id: WatchId) -> Option<&mut WatchCore> {
        self.cores.get_mut(&id)
    }

    /// Aktywne obserwacje.
    pub fn specs(&self) -> Vec<(WatchId, WatchSpec)> {
        self.cores
            .iter()
            .map(|(i, c)| (*i, c.spec().clone()))
            .collect()
    }

    /// Surowa zmiana dla obserwacji.
    pub fn raw(&mut self, id: WatchId, now_ms: u64, change: RawChange) {
        if let Some(core) = self.cores.get_mut(&id) {
            core.raw(now_ms, change);
        }
    }

    /// Pełne przeskanowanie (zdarzenie `Rescanned` od razu w kolejce).
    pub fn rescan(
        &mut self,
        id: WatchId,
        now_ms: u64,
        reason: RescanReason,
        listing: Vec<(PathBuf, FileStamp)>,
    ) {
        if let Some(core) = self.cores.get_mut(&id) {
            let ev = core.rescan(now_ms, reason, listing);
            self.queue.push_back(ev);
        }
    }

    /// Obserwacja zakończona przez system (zdarzenie `Stopped`).
    pub fn stopped(&mut self, id: WatchId, reason: &str) {
        if self.cores.remove(&id).is_some() {
            self.queue.push_back(WatchEvent::Stopped {
                watch: id,
                reason: reason.chars().take(200).collect(),
            });
        }
    }

    /// Zdarzenia gotowe w chwili `now_ms`.
    pub fn poll(&mut self, now_ms: u64) -> Vec<WatchEvent> {
        let mut out: Vec<WatchEvent> = self.queue.drain(..).collect();
        for core in self.cores.values_mut() {
            out.extend(core.poll(now_ms, false));
        }
        out
    }

    /// Najbliższa chwila gotowości (kolejka niepusta = teraz).
    pub fn next_due_ms(&self, now_ms: u64) -> Option<u64> {
        if !self.queue.is_empty() {
            return Some(now_ms);
        }
        self.cores.values().filter_map(WatchCore::next_due_ms).min()
    }
}
