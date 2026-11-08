//! Atrapa `skills` (docs/modules/skills/SPEC.md, „Fake”): rdzeń [`SkillLibrary`] z kontraktu,
//! magazyn w pamięci, **wirtualny zegar** (czas płynie tylko przez [`FakeSkills::advance`]),
//! zdarzenia nagrywane zamiast magistrali. Do testów UI, `agent-builder`, `app-*` — wyłącznie
//! jako dev-dependency.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::Event;
use personas_contract::Role;
use semver::Version;
use skills_contract::{
    ImportOrigin, ImportReport, OwnerApproval, Skill, SkillBundle, SkillError, SkillId,
    SkillLibrary, SkillMatch, SkillRecord, SkillSource, Skills, search,
};
use tools_common_contract::ToolManifest;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Atrapa biblioteki umiejętności.
pub struct FakeSkills {
    lib: Mutex<SkillLibrary>,
    events: Mutex<Vec<Event>>,
    clock: AtomicU64,
}

impl FakeSkills {
    /// Biblioteka nad katalogiem narzędzi (czas 0).
    pub fn new(catalog: Vec<ToolManifest>) -> Self {
        Self {
            lib: Mutex::new(SkillLibrary::new(catalog, Vec::new())),
            events: Mutex::new(Vec::new()),
            clock: AtomicU64::new(0),
        }
    }

    /// Przesuwa wirtualny zegar.
    pub fn advance(&self, ms: u64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    /// Nagrane zdarzenia.
    pub fn events(&self) -> Vec<Event> {
        lock(&self.events).clone()
    }

    fn now(&self) -> u64 {
        self.clock.load(Ordering::SeqCst)
    }

    fn apply<T>(
        &self,
        f: impl FnOnce(&mut SkillLibrary, u64) -> Result<(T, Vec<Event>), SkillError>,
    ) -> Result<T, SkillError> {
        let now = self.now();
        let (value, ev) = f(&mut lock(&self.lib), now)?;
        lock(&self.events).extend(ev);
        Ok(value)
    }
}

#[async_trait]
impl Skills for FakeSkills {
    fn catalog(&self) -> Vec<ToolManifest> {
        lock(&self.lib).catalog().to_vec()
    }

    fn list(&self) -> Vec<SkillRecord> {
        lock(&self.lib).records().to_vec()
    }

    fn installed(&self, id: &SkillId) -> Option<SkillRecord> {
        lock(&self.lib).installed(id).cloned()
    }

    async fn propose(&self, skill: Skill, source: SkillSource) -> Result<SkillRecord, SkillError> {
        self.apply(|l, now| l.propose(skill, source, now))
    }

    async fn approve(
        &self,
        id: &SkillId,
        version: &Version,
        approval: OwnerApproval,
    ) -> Result<SkillRecord, SkillError> {
        self.apply(|l, now| l.approve(id, version, approval, now))
    }

    async fn release(
        &self,
        id: &SkillId,
        version: &Version,
        approval: OwnerApproval,
    ) -> Result<SkillRecord, SkillError> {
        self.apply(|l, now| l.release(id, version, approval, now))
    }

    async fn reject(&self, id: &SkillId, version: &Version) -> Result<SkillRecord, SkillError> {
        self.apply(|l, now| l.reject(id, version, now))
    }

    async fn disable(&self, id: &SkillId) -> Result<SkillRecord, SkillError> {
        self.apply(|l, now| l.disable(id, now))
    }

    fn search(&self, task: &str, caller_roles: &[Role], limit: usize) -> Vec<SkillMatch> {
        let l = lock(&self.lib);
        search(l.records(), l.catalog(), caller_roles, task, limit)
    }

    fn export(&self, ids: &[SkillId]) -> Result<SkillBundle, SkillError> {
        lock(&self.lib).export(ids)
    }

    async fn import(
        &self,
        bundle: &SkillBundle,
        origin: ImportOrigin,
    ) -> Result<ImportReport, SkillError> {
        self.apply(|l, now| l.import(bundle, origin, now))
    }
}
