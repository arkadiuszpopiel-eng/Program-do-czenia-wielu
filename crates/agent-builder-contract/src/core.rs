//! Rdzeń Kreatora (wspólny dla `-impl` i `-fake`): budowa, podgląd, test na sucho (rejestr
//! zaliczonych hashy), zapis dwufazowy — `prepare_save` (pełna ponowna walidacja: manifest musi
//! być dokładnie tym, co zbudowałby Kreator; hash = przejrzany; zaliczony test na sucho; kanał
//! zatwierdzenia ≠ głos) i `commit` (katalog person, głosy, biblioteka).

use std::collections::BTreeSet;

use core_bus_contract::{Event, Level};
use personas_contract::{Catalog, PersonaId, RoleId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::ToolManifest;
use voice_tts_contract::{PIPER_BASE, TtsEngine, VoiceRef};

use crate::build::{BuildContext, Built, build_manifest};
use crate::draft::{
    AgentDraft, AgentManifest, BuilderApproval, BuilderApprovalOrigin, DryScenario,
};
use crate::dry_run::{DryRunReport, Preview, dry_run, preview};
use crate::manifest::draft_of;
use crate::policy::{BuildError, BuilderPolicy};
use crate::{event_kind, events};

/// Zapisana agentka.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SavedAgent {
    /// Persona.
    pub persona: PersonaId,
    /// Rola.
    pub role: RoleId,
    /// Hash manifestu.
    pub hash: String,
}

fn engine_of(m: &AgentManifest) -> TtsEngine {
    if m.voice.base_speaker == PIPER_BASE {
        TtsEngine::Piper
    } else {
        TtsEngine::Pocket
    }
}

/// Stan Kreatora.
#[derive(Debug, Clone)]
pub struct BuilderCore {
    policy: BuilderPolicy,
    tools: Vec<ToolManifest>,
    catalog: Catalog,
    voices: Vec<(PersonaId, Vec<VoiceRef>)>,
    passed: BTreeSet<String>,
    library: Vec<AgentManifest>,
}

impl BuilderCore {
    /// Kreator nad polityką, katalogiem narzędzi, katalogiem person i istniejącymi głosami.
    pub fn new(
        policy: BuilderPolicy,
        tools: Vec<ToolManifest>,
        catalog: Catalog,
        voices: Vec<(PersonaId, Vec<VoiceRef>)>,
    ) -> Self {
        Self {
            policy,
            tools,
            catalog,
            voices,
            passed: BTreeSet::new(),
            library: Vec::new(),
        }
    }

    /// Sufit autonomii z bieżącej sesji (Broker); zawsze ≤ L3.
    pub fn set_ceiling(&mut self, level: risk_classifier_contract::AutonomyLevel) {
        self.policy.ceiling = level.min(risk_classifier_contract::AutonomyLevel::L3);
    }

    /// Katalog person i ról (odświeżenie z usługi `personas`).
    pub fn set_catalog(&mut self, catalog: Catalog) {
        self.catalog = catalog;
    }

    /// Przywrócenie zapisanych manifestów (z własnego magazynu): biblioteka i ich głosy.
    pub fn restore(&mut self, manifests: Vec<AgentManifest>) {
        for m in manifests {
            self.voices.push((
                m.persona.id.clone(),
                vec![VoiceRef {
                    persona: m.persona.id.clone(),
                    engine: engine_of(&m),
                    preset: m.voice.clone(),
                    reference: None,
                }],
            ));
            self.library.push(m);
        }
    }

    /// Polityka.
    pub fn policy(&self) -> &BuilderPolicy {
        &self.policy
    }

    /// Katalog narzędzi.
    pub fn tools(&self) -> &[ToolManifest] {
        &self.tools
    }

    /// Zapisane manifesty.
    pub fn library(&self) -> &[AgentManifest] {
        &self.library
    }

    /// Budowa manifestu ze szkicu.
    pub fn build(&self, draft: &AgentDraft) -> Result<Built, BuildError> {
        let ctx = BuildContext {
            catalog: &self.catalog,
            voices: &self.voices,
        };
        build_manifest(draft, &self.policy, &ctx)
    }

    /// Ponowna walidacja manifestu (pola publiczne — mógł zostać zmieniony poza Kreatorem).
    pub fn revalidate(&self, m: &AgentManifest) -> Result<Built, BuildError> {
        let built = self.build(&draft_of(m))?;
        if &built.manifest != m {
            return Err(BuildError::Invalid(
                "manifest zmieniony poza Kreatorem — zbuduj go ponownie".into(),
            ));
        }
        Ok(built)
    }

    /// Podgląd.
    pub fn preview(&self, m: &AgentManifest) -> Preview {
        preview(m, &self.tools)
    }

    /// Test na sucho; zaliczony zapamiętuje hash manifestu.
    pub fn dry_run(
        &mut self,
        m: &AgentManifest,
        scenario: &DryScenario,
    ) -> Result<(DryRunReport, Vec<Event>), BuildError> {
        let built = self.revalidate(m)?;
        let report = dry_run(m, &self.tools, scenario);
        if report.passed {
            self.passed.insert(built.hash.clone());
        }
        let ev = Event::new(
            event_kind(events::DRY_RUN),
            Level::Info,
            serde_json::json!({ "persona": m.persona.id, "hash": built.hash, "passed": report.passed, "steps": report.steps.len() }),
        );
        Ok((report, vec![ev]))
    }

    /// Faza 1 zapisu: wszystkie warunki (bez zmiany stanu).
    pub fn prepare_save(
        &self,
        m: &AgentManifest,
        approval: &BuilderApproval,
    ) -> Result<Built, BuildError> {
        if approval.origin == BuilderApprovalOrigin::Voice {
            return Err(BuildError::Approval(
                "zapis nowej agentki zatwierdzasz w oknie albo tekstem, nie głosem".into(),
            ));
        }
        let built = self.revalidate(m)?;
        if approval.reviewed_hash != built.hash {
            return Err(BuildError::Approval(
                "przejrzana wersja różni się od zapisywanej".into(),
            ));
        }
        if !self.passed.contains(&built.hash) {
            return Err(BuildError::DryRunRequired);
        }
        Ok(built)
    }

    /// Faza 2 zapisu: katalog, głosy, biblioteka (po udanym zapisie w usłudze person).
    pub fn commit(&mut self, built: Built) -> Result<(SavedAgent, Vec<Event>), BuildError> {
        let m = built.manifest;
        self.catalog
            .add_role(m.role.clone())
            .map_err(|e| BuildError::Conflict(e.to_string()))?;
        self.catalog
            .add_persona(m.persona.clone())
            .map_err(|e| BuildError::Conflict(e.to_string()))?;
        self.voices.push((
            m.persona.id.clone(),
            vec![VoiceRef {
                persona: m.persona.id.clone(),
                engine: engine_of(&m),
                preset: m.voice.clone(),
                reference: None,
            }],
        ));
        self.passed.remove(&built.hash);
        let saved = SavedAgent {
            persona: m.persona.id.clone(),
            role: m.role.id.clone(),
            hash: built.hash,
        };
        let ev = Event::new(
            event_kind(events::SAVED),
            Level::Info,
            serde_json::json!({ "persona": saved.persona, "role": saved.role, "hash": saved.hash, "autonomy": m.limits.autonomy, "tools": m.role.tools }),
        );
        self.library.push(m);
        Ok((saved, vec![ev]))
    }
}
