//! Kontrakt modułu `agent-builder` — Kreator agentów (docs/modules/agent-builder/SPEC.md,
//! PLAN §9.2, §9.5, §12.1 pierścień R1).
//!
//! Opis słowami albo formularz → szkic ([`AgentDraft`], [`from_description`], port modelu
//! [`DraftLlm`]) → **zwalidowany manifest** ([`AgentManifest`]: persona z odmianą imienia
//! [`decline_feminine`], kolor z palety, głos v0 odrębny od istniejących, rola z promptem
//! w rodzaju żeńskim, grupy narzędzi z listy dozwolonych, limity) → podgląd i **test na sucho**
//! ([`DryScenario`], skryptowany przebieg bez modelu i bez skutków) → **zapis po zatwierdzeniu**
//! ([`BuilderApproval`] z hashem przejrzanego manifestu; nie głosem).
//!
//! Twarde reguły ([`BuilderPolicy`]): nic spoza listy dozwolonych grup narzędzi, żadnych
//! uprawnień Jądra (Broker, audyt, autonomia, polityki, sekrety, administracja — także
//! w nazwach i promptach), autonomia ≤ poziom tworzącej sesji i nigdy L4, budżet ≤ sufit,
//! zapis tylko w profilu użytkownika (bez katalogów Alfy, systemu i poświadczeń).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod build;
mod core;
mod decline;
mod draft;
mod dry_run;
mod manifest;
mod parse;
mod policy;
pub mod samples;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use async_trait::async_trait;
use core_bus_contract::EventKind;

pub use crate::core::{BuilderCore, SavedAgent};
pub use build::{BuildContext, Built, MODEL_POLICIES, build_manifest};
pub use decline::decline_feminine;
pub use draft::{
    AgentDraft, AgentLimits, AgentManifest, BuilderApproval, BuilderApprovalOrigin, DraftProposal,
    DryExpect, DryScenario, DryStep, LimitsDraft, RoleDraft, VoiceDraft,
};
pub use dry_run::{DryRunReport, DryStepResult, Preview, dry_run, in_scope, preview};
pub use manifest::{draft_of, manifest_hash};
pub use parse::{DraftLlm, from_conversation, from_description};
pub use policy::{
    BuildError, BuilderPolicy, FORBIDDEN_PATH_PARTS, KERNEL_MARKERS, PROMPT_PHRASES, PROMPT_TOKENS,
    check_feminine, check_fs_scope, check_kernel_id, check_text,
};

/// Nazwy zdarzeń.
pub mod events {
    /// Test na sucho (hash, wynik).
    pub const DRY_RUN: &str = "agent_builder.dry_run";
    /// Zapisano agentkę (persona, rola, hash, autonomia, grupy narzędzi).
    pub const SAVED: &str = "agent_builder.saved";
}

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Kreator agentów. Metody nie są narzędziami agentek — wołają je komendy UI właściciela.
#[async_trait]
pub trait AgentBuilder: Send + Sync {
    /// Polityka (paleta, sufity, dozwolone grupy) — dla formularza.
    fn policy(&self) -> BuilderPolicy;

    /// Szkic z opisu (deterministycznie) + pytania o braki.
    fn propose(&self, description: &str) -> DraftProposal {
        from_description(description)
    }

    /// Szkic z opisu z pomocą modelu (uzupełnia braki; autonomia niższa z dwóch).
    async fn propose_with(&self, description: &str, llm: &dyn DraftLlm) -> DraftProposal {
        from_conversation(description, llm).await
    }

    /// Budowa i walidacja manifestu.
    fn build(&self, draft: &AgentDraft) -> Result<Built, BuildError>;

    /// Podgląd (prompt, narzędzia, głos, autonomia).
    fn preview(&self, manifest: &AgentManifest) -> Preview;

    /// Test na sucho (zaliczony = warunek zapisu tego samego manifestu).
    async fn dry_run(
        &self,
        manifest: &AgentManifest,
        scenario: &DryScenario,
    ) -> Result<DryRunReport, BuildError>;

    /// Zapis po zatwierdzeniu właściciela (persona i rola trafiają do katalogu `personas`).
    async fn save(
        &self,
        manifest: &AgentManifest,
        approval: BuilderApproval,
    ) -> Result<SavedAgent, BuildError>;

    /// Zapisane manifesty.
    fn library(&self) -> Vec<AgentManifest>;
}
