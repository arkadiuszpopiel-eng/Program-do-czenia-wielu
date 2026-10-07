//! Kontrakt modułu `skills` — umiejętności agentek (docs/modules/skills/SPEC.md, PLAN §9.2,
//! §9.5, §12.1 pierścień R1, §8.0 „zatruta umiejętność”).
//!
//! Umiejętność = nazwany, wersjonowany przepis ([`Skill`]): opis, wymagane narzędzia
//! i zdolności (zgodne z manifestami, bez zdolności Jądra), parametry z JSON Schema, szablon
//! polecenia i kroki, przykłady, testy akceptacyjne. Źródła ([`SkillSource`]): pamięć
//! proceduralna ([`draft_from_memory`]), właściciel, import. **Instalacja i aktualizacja tylko
//! po zatwierdzeniu właściciela** ([`OwnerApproval`] z hashem przejrzanej treści); treść
//! niezaufana (pamięć z niezaufanej proweniencji, import z zewnątrz) albo podejrzana trafia do
//! **kwarantanny** (zwolnienie wyłącznie w oknie). Uprawnienia umiejętności ≤ roli wywołującej
//! ([`runnable_by`], [`prepare_run`] — koperta przebiegu `agent-runtime` = wymagania ∩ koperta
//! wywołującej). Wyszukiwanie deterministyczne ([`search`]) + port modelu ([`SkillRanker`],
//! [`rerank`]). Eksport/import: paczka JSON `alfa.skills.v1` z SHA-256 ([`SkillBundle`]).
//!
//! Rdzeń stanu ([`SkillLibrary`]) jest wspólny dla `-impl` i `-fake` (różnią się magazynem
//! i ujściem zdarzeń), więc oba przechodzą ten sam test kontraktowy.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod bundle;
mod error;
mod library;
mod memory;
mod model;
mod run;
pub mod samples;
pub mod schema;
mod search;
mod validate;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use agent_runtime_contract::{RunId, RunOptions, RunSpec};
use async_trait::async_trait;
use core_bus_contract::EventKind;
use memory_contract::MemoryEntry;
use personas_contract::Role;
use semver::Version;
use tools_common_contract::ToolManifest;

pub use bundle::{
    BUNDLE_DOCUMENT, BUNDLE_FORMAT, BundledSkill, MAX_BUNDLE_BYTES, SkillBundle, canonical_json,
    content_hash,
};
pub use error::SkillError;
pub use library::{ImportReport, SkillLibrary};
pub use memory::draft_from_memory;
pub use model::{
    AcceptanceTest, ApprovalOrigin, ImportOrigin, OwnerApproval, Skill, SkillExample, SkillId,
    SkillRecord, SkillSource, SkillState,
};
pub use run::{prepare_run, runnable_by};
pub use search::{MIN_SCORE, RERANK_TOP, SkillMatch, SkillRanker, rerank, score, search, stems};
pub use validate::{
    FORBIDDEN_CAPABILITIES, check_tools, render_goal, run_acceptance, scan, validate_skill,
};

/// Nazwy zdarzeń (ładunki bez treści przepisów: identyfikator, wersja, hash, stan).
pub mod events {
    /// Nowa propozycja czeka na zatwierdzenie.
    pub const PROPOSED: &str = "skills.proposed";
    /// Propozycja w kwarantannie.
    pub const QUARANTINED: &str = "skills.quarantined";
    /// Zainstalowano wersję.
    pub const INSTALLED: &str = "skills.installed";
    /// Zwolniono z kwarantanny (poziom `Warn` — w Audycie).
    pub const RELEASED: &str = "skills.released";
    /// Odrzucono.
    pub const REJECTED: &str = "skills.rejected";
    /// Wyłączono.
    pub const DISABLED: &str = "skills.disabled";
    /// Starsza wersja zastąpiona.
    pub const SUPERSEDED: &str = "skills.superseded";
    /// Zaimportowano paczkę (liczniki).
    pub const IMPORTED: &str = "skills.imported";
}

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Biblioteka umiejętności. Metody zmieniające stan publikują zdarzenia `skills.*`. Żadna
/// z nich nie jest narzędziem agentki — zatwierdzenia przychodzą wyłącznie z UI właściciela.
#[async_trait]
pub trait Skills: Send + Sync {
    /// Katalog narzędzi (walidacja wymagań).
    fn catalog(&self) -> Vec<ToolManifest>;

    /// Wszystkie wersje (Inspektor umiejętności).
    fn list(&self) -> Vec<SkillRecord>;

    /// Zainstalowana wersja.
    fn installed(&self, id: &SkillId) -> Option<SkillRecord>;

    /// Propozycja (walidacja, testy akceptacyjne, skaner; niezaufane → kwarantanna).
    async fn propose(&self, skill: Skill, source: SkillSource) -> Result<SkillRecord, SkillError>;

    /// Zatwierdzenie propozycji → instalacja (aktualizacja: starsza wersja „zastąpiona”).
    async fn approve(
        &self,
        id: &SkillId,
        version: &Version,
        approval: OwnerApproval,
    ) -> Result<SkillRecord, SkillError>;

    /// Zwolnienie z kwarantanny (tylko `ApprovalOrigin::Ui`) → instalacja.
    async fn release(
        &self,
        id: &SkillId,
        version: &Version,
        approval: OwnerApproval,
    ) -> Result<SkillRecord, SkillError>;

    /// Odrzucenie propozycji albo kwarantanny.
    async fn reject(&self, id: &SkillId, version: &Version) -> Result<SkillRecord, SkillError>;

    /// Wyłączenie zainstalowanej.
    async fn disable(&self, id: &SkillId) -> Result<SkillRecord, SkillError>;

    /// Umiejętności pasujące do zadania i dozwolone dla ról wywołującej.
    fn search(&self, task: &str, caller_roles: &[Role], limit: usize) -> Vec<SkillMatch>;

    /// Przebieg umiejętności dla wywołującej (koperta ≤ wywołującej).
    fn prepare_run(
        &self,
        id: &SkillId,
        params: &serde_json::Value,
        caller: &RunSpec,
        caller_options: &RunOptions,
        parent: Option<RunId>,
    ) -> Result<(RunSpec, RunOptions), SkillError> {
        let record = self
            .installed(id)
            .ok_or_else(|| SkillError::NotFound(id.clone()))?;
        prepare_run(
            &record,
            params,
            &self.catalog(),
            caller,
            caller_options,
            parent,
        )
    }

    /// Eksport zainstalowanych (puste `ids` = wszystkie).
    fn export(&self, ids: &[SkillId]) -> Result<SkillBundle, SkillError>;

    /// Import paczki jako propozycji.
    async fn import(
        &self,
        bundle: &SkillBundle,
        origin: ImportOrigin,
    ) -> Result<ImportReport, SkillError>;

    /// Propozycja z wpisu pamięci proceduralnej.
    async fn propose_from_memory(&self, entry: &MemoryEntry) -> Result<SkillRecord, SkillError> {
        let (skill, source) = draft_from_memory(entry, &self.catalog())?;
        self.propose(skill, source).await
    }
}
