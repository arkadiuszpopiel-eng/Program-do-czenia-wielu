//! Komendy Kreatora agentów (`builder_*`): rozmowa (opis → szkic + pytania, deterministycznie)
//! albo formularz → manifest (walidacja Kreatora: imię z odmianą, kolor z palety, głos v0, rola
//! w rodzaju żeńskim, grupy narzędzi z listy, autonomia ≤ sufit z Brokera — nigdy L4) → podgląd →
//! test na sucho (scenariusz z limitów manifestu) → zapis tylko z UI (nie głosem) po zaliczonym
//! teście tego samego hasha.

use std::sync::Arc;

use agent_builder_contract::{
    AgentBuilder, AgentManifest, BuildError, BuilderApproval, BuilderApprovalOrigin, DryExpect,
    DryRunReport, DryScenario, DryStep, MODEL_POLICIES,
};
use agent_builder_impl::AgentBuilderModule;
use app_api::AppError;
use app_api::dto::{
    AgentDraft, AutonomyLevel, BuilderAgentInfo, BuilderDryRun, BuilderDryStep, BuilderPolicyView,
    BuilderPreview, BuilderProposal, BuilderSaved, DryOutcome,
};
use app_api::ports::VoicePort;
use risk_classifier_contract::{AutonomyLevel as Level, Reversibility};
use tools_common_contract::ToolManifest;

/// Zakres poza profilem agentki (test na sucho: zapis musi być odrzucony).
const OUTSIDE: &str = "%USERPROFILE%\\.ssh\\alfa-test-na-sucho";

pub(crate) fn level(l: Level) -> AutonomyLevel {
    match l {
        Level::L0 => AutonomyLevel::L0,
        Level::L1 => AutonomyLevel::L1,
        Level::L2 => AutonomyLevel::L2,
        Level::L3 => AutonomyLevel::L3,
        Level::L4 => AutonomyLevel::L4,
    }
}

fn outcome(e: DryExpect) -> DryOutcome {
    match e {
        DryExpect::Allowed => DryOutcome::Allowed,
        DryExpect::Ask => DryOutcome::Ask,
        DryExpect::Denied => DryOutcome::Denied,
    }
}

fn build_error(e: BuildError) -> AppError {
    AppError::invalid(format!("Kreator: {e}"))
}

/// Ścieżka w pierwszym zakresie zapisu (`prefiks\**` → `prefiks\alfa-test.txt`).
fn inside(scope: &str) -> String {
    let base = scope
        .trim_end_matches("**")
        .trim_end_matches(['\\', '/'])
        .to_owned();
    format!("{base}\\alfa-test-na-sucho.txt")
}

/// Scenariusz testu na sucho z limitów manifestu (intencja właściciela, nie wynik silnika):
/// odczyty roli — dozwolone; zapis w zakresie — wg autonomii; zapis poza zakresem i narzędzie
/// spoza roli — odmowa.
pub fn default_scenario(m: &AgentManifest, catalog: &[ToolManifest]) -> DryScenario {
    let role = &m.role;
    let allowed: Vec<&ToolManifest> = catalog
        .iter()
        .filter(|t| t.allowed_for(&role.tools, role.read_only))
        .collect();
    let mut steps: Vec<DryStep> = allowed
        .iter()
        .filter(|t| !t.mutating)
        .take(3)
        .map(|t| DryStep {
            tool: t.name.clone(),
            args: serde_json::json!({}),
            expect: DryExpect::Allowed,
        })
        .collect();
    let writer = allowed
        .iter()
        .find(|t| t.mutating && t.capabilities.iter().any(|c| c == "fs.write"));
    if let Some(w) = writer {
        if let Some(scope) = m.limits.fs_write.first() {
            let destructive = w.reversible != Reversibility::Yes;
            let expect = match m.limits.autonomy {
                Level::L0 => DryExpect::Denied,
                Level::L1 => DryExpect::Ask,
                Level::L2 if destructive => DryExpect::Ask,
                _ => DryExpect::Allowed,
            };
            steps.push(DryStep {
                tool: w.name.clone(),
                args: serde_json::json!({ "path": inside(scope) }),
                expect,
            });
        }
        steps.push(DryStep {
            tool: w.name.clone(),
            args: serde_json::json!({ "path": OUTSIDE }),
            expect: DryExpect::Denied,
        });
    }
    if let Some(other) = catalog
        .iter()
        .find(|t| !t.allowed_for(&role.tools, role.read_only))
    {
        steps.push(DryStep {
            tool: other.name.clone(),
            args: serde_json::json!({}),
            expect: DryExpect::Denied,
        });
    }
    DryScenario { steps }
}

fn dry_view(hash: String, report: &DryRunReport) -> BuilderDryRun {
    BuilderDryRun {
        hash,
        passed: report.passed,
        steps: report
            .steps
            .iter()
            .map(|s| BuilderDryStep {
                tool: s.tool.clone(),
                expected: outcome(s.expected),
                outcome: outcome(s.outcome),
                why: s.why.clone(),
            })
            .collect(),
    }
}

/// Kreator w aplikacji.
pub struct BuilderApp {
    module: Result<Arc<AgentBuilderModule>, String>,
    catalog: Vec<ToolManifest>,
    voice: Arc<dyn VoicePort>,
}

impl std::fmt::Debug for BuilderApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuilderApp")
            .field("tools", &self.catalog.len())
            .finish_non_exhaustive()
    }
}

impl BuilderApp {
    /// Kreator nad uruchomionym modułem (albo błędem jego budowy) i katalogiem narzędzi agentek.
    pub fn new(
        module: Result<Arc<AgentBuilderModule>, String>,
        catalog: Vec<ToolManifest>,
        voice: Arc<dyn VoicePort>,
    ) -> Self {
        Self {
            module,
            catalog,
            voice,
        }
    }

    /// Moduł (rejestr).
    pub fn module(&self) -> Result<Arc<AgentBuilderModule>, String> {
        self.module.clone()
    }

    fn m(&self) -> Result<&Arc<AgentBuilderModule>, AppError> {
        self.module
            .as_ref()
            .map_err(|_| AppError::unavailable("Kreator agentek", "agent-builder"))
    }

    /// `builder_policy`.
    pub fn policy(&self) -> Result<BuilderPolicyView, AppError> {
        let p = self.m()?.policy();
        Ok(BuilderPolicyView {
            groups: p.allowed_groups.into_iter().collect(),
            ceiling: level(p.ceiling),
            palette: p.palette,
            voices: voice_tts_contract::POCKET_BASE
                .iter()
                .map(|v| (*v).to_owned())
                .chain(std::iter::once(voice_tts_contract::PIPER_BASE.to_owned()))
                .collect(),
            model_policies: MODEL_POLICIES.iter().map(|m| (*m).to_owned()).collect(),
            max_steps: p.max_budget.max_steps,
        })
    }

    /// `builder_propose`: opis słowami → szkic + pytania o braki.
    pub fn propose(&self, description: &str) -> Result<BuilderProposal, AppError> {
        let text = description.trim();
        if text.is_empty() || text.chars().count() > 4_000 {
            return Err(AppError::invalid(
                "Opis agentki musi mieć od 1 do 4000 znaków.",
            ));
        }
        let p = self.m()?.propose(text);
        Ok(BuilderProposal {
            draft: p.draft,
            questions: p.questions,
        })
    }

    fn built(&self, draft: &AgentDraft) -> Result<agent_builder_contract::Built, AppError> {
        self.m()?.build(draft).map_err(build_error)
    }

    /// `builder_preview`: manifest zbudowany ze szkicu (walidacja Kreatora).
    pub fn preview(&self, draft: &AgentDraft) -> Result<BuilderPreview, AppError> {
        let built = self.built(draft)?;
        let m = &built.manifest;
        let p = self.m()?.preview(m);
        let f = &m.persona.forms;
        Ok(BuilderPreview {
            hash: built.hash.clone(),
            persona_id: m.persona.id.to_string(),
            name: m.persona.name.clone(),
            forms: [
                &f.nominative,
                &f.genitive,
                &f.dative,
                &f.accusative,
                &f.instrumental,
                &f.locative,
                &f.vocative,
            ]
            .iter()
            .map(|s| (*s).clone())
            .collect(),
            glyph: m.persona.glyph.to_string(),
            color: m.persona.color.to_string(),
            character: m.persona.character.clone(),
            role_id: m.role.id.to_string(),
            role_name: m.role.name.clone(),
            groups: m.role.tools.clone(),
            read_only: m.role.read_only,
            tools: p.tools,
            voice: p.voice,
            autonomy: level(p.autonomy),
            system_prompt: p.system_prompt,
            fs_write: m.limits.fs_write.clone(),
            memory_scope: m.limits.memory_scope.clone(),
            retain_days: m.limits.retain_days,
            max_steps: m.limits.budget.max_steps,
            warnings: built.warnings,
        })
    }

    /// `builder_dry_run`: test na sucho manifestu (bez modelu i skutków).
    pub async fn dry_run(&self, draft: &AgentDraft) -> Result<BuilderDryRun, AppError> {
        let built = self.built(draft)?;
        let scenario = default_scenario(&built.manifest, &self.catalog);
        let report = self
            .m()?
            .dry_run(&built.manifest, &scenario)
            .await
            .map_err(build_error)?;
        Ok(dry_view(built.hash, &report))
    }

    /// `builder_save`: zapis po zatwierdzeniu w UI (hash = przejrzany manifest z zaliczonym testem).
    pub async fn save(&self, draft: &AgentDraft, hash: &str) -> Result<BuilderSaved, AppError> {
        let built = self.built(draft)?;
        if built.hash != hash.trim() {
            return Err(AppError::invalid(
                "Szkic zmienił się od podglądu — obejrzyj podgląd i wykonaj test na sucho ponownie.",
            ));
        }
        let approval = BuilderApproval {
            origin: BuilderApprovalOrigin::Ui,
            reviewed_hash: built.hash.clone(),
        };
        let saved = self
            .m()?
            .save(&built.manifest, approval)
            .await
            .map_err(build_error)?;
        Ok(BuilderSaved {
            persona: saved.persona.to_string(),
            role: saved.role.to_string(),
            hash: saved.hash,
        })
    }

    /// `builder_voice_preview`: odsłuch mówczyni bazowej szkicu (głos v0 bez korekty wysokości
    /// i tempa — łańcuch TTS nowej agentki powstaje po zapisie).
    pub async fn voice_preview(&self, draft: &AgentDraft) -> Result<(), AppError> {
        let base = draft
            .voice
            .as_ref()
            .map(|v| v.base.as_str())
            .unwrap_or("pl-f1");
        let speaker = if base == voice_tts_contract::POCKET_BASE[1] {
            "beta"
        } else {
            "alfa"
        };
        let name = draft.name.as_deref().unwrap_or("nowa agentka");
        let text = format!("Cześć, jestem {name}. Tak mniej więcej brzmi mój głos.");
        self.voice.read_aloud(speaker, &text).await
    }

    /// `builder_library`: agentki zapisane Kreatorem.
    pub fn library(&self) -> Result<Vec<BuilderAgentInfo>, AppError> {
        Ok(self
            .m()?
            .library()
            .iter()
            .map(|m| BuilderAgentInfo {
                persona: m.persona.id.to_string(),
                name: m.persona.name.clone(),
                color: m.persona.color.to_string(),
                role: m.role.name.clone(),
                autonomy: level(m.limits.autonomy),
                hash: agent_builder_contract::manifest_hash(m).unwrap_or_default(),
            })
            .collect())
    }
}
