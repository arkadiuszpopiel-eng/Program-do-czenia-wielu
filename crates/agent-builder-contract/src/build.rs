//! Szkic → zwalidowany manifest: persona (imię z odmianą, glif, kolor z palety, charakter,
//! biblia głosu), rola (grupy narzędzi z listy dozwolonych, prompt żeński bez poleceń obejścia),
//! głos v0 (odrębny od istniejących), limity (autonomia ≤ sufit, budżet ≤ sufit, zakresy zapisu
//! w profilu). Kolizje sprawdza katalog `personas`. Hash manifestu = treść do zatwierdzenia.

use personas_contract::{
    Catalog, ColorToken, DEFAULT_PROMPT_TEMPLATE, Persona, PersonaId, Role, RoleId, VoiceBible,
    fold, is_valid_id, render_system_prompt,
};
use risk_classifier_contract::AutonomyLevel;
use voice_tts_contract::{
    PIPER_BASE, POCKET_BASE, TtsEngine, VoicePreset, VoiceRef, validate_chains,
};

use crate::decline::decline_feminine;
use crate::draft::{AgentDraft, AgentLimits, AgentManifest, VoiceDraft};
use crate::manifest::manifest_hash;
use crate::policy::{
    BuildError, BuilderPolicy, check_feminine, check_fs_scope, check_kernel_id, check_text,
};

/// Klasy zadań dla routera dozwolone w rolach własnych.
pub const MODEL_POLICIES: [&str; 7] = [
    "conversation",
    "planning",
    "gui-vision",
    "code",
    "review",
    "research",
    "summarize",
];

/// Kontekst budowy: katalog person (kolizje) i istniejące głosy (odrębność brzmienia).
#[derive(Debug, Clone, Copy)]
pub struct BuildContext<'a> {
    /// Katalog person i ról (wbudowane + własne).
    pub catalog: &'a Catalog,
    /// Łańcuchy głosów istniejących agentek.
    pub voices: &'a [(PersonaId, Vec<VoiceRef>)],
}

/// Wynik budowy.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    /// Manifest.
    pub manifest: AgentManifest,
    /// Hash (SHA-256 hex) — do zatwierdzenia zapisu.
    pub hash: String,
    /// Ostrzeżenia dla właściciela.
    pub warnings: Vec<String>,
}

fn field(name: &str, value: &Option<String>, max: usize) -> Result<String, BuildError> {
    let v = value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| BuildError::Missing(name.to_owned()))?;
    if v.chars().count() > max {
        return Err(BuildError::Invalid(name.to_owned()));
    }
    check_text(v)?;
    Ok(v.to_owned())
}

fn persona_id(name: &str, explicit: Option<&str>) -> Result<PersonaId, BuildError> {
    let raw = explicit.map_or_else(|| fold(name).replace(' ', "-"), str::to_owned);
    check_kernel_id(&raw)?;
    PersonaId::parse(&raw).ok_or_else(|| BuildError::Invalid("identyfikator agentki".into()))
}

fn voice(
    draft: Option<&VoiceDraft>,
    id: &PersonaId,
    ctx: &BuildContext<'_>,
) -> Result<(VoicePreset, VoiceBible), BuildError> {
    let candidates: Vec<VoicePreset> = match draft {
        Some(v) => vec![VoicePreset {
            base_speaker: v.base.clone(),
            pitch: v.pitch,
            rate: v.rate,
        }],
        None => [1.03f32, 0.97, 1.09, 0.95, 1.15, 0.88]
            .iter()
            .flat_map(|p| {
                POCKET_BASE.iter().map(move |b| VoicePreset {
                    base_speaker: (*b).to_owned(),
                    pitch: *p,
                    rate: 1.0,
                })
            })
            .collect(),
    };
    let mut last = BuildError::Missing("głos".into());
    for preset in candidates {
        if !POCKET_BASE.contains(&preset.base_speaker.as_str()) && preset.base_speaker != PIPER_BASE
        {
            return Err(BuildError::Invalid("mówczyni bazowa głosu".into()));
        }
        let mut chains = ctx.voices.to_vec();
        let engine = if preset.base_speaker == PIPER_BASE {
            TtsEngine::Piper
        } else {
            TtsEngine::Pocket
        };
        chains.push((
            id.clone(),
            vec![VoiceRef {
                persona: id.clone(),
                engine,
                preset: preset.clone(),
                reference: None,
            }],
        ));
        match validate_chains(&chains) {
            Ok(()) => {
                let age = draft.map_or(22, |v| v.perceived_age);
                let timbre =
                    draft.map_or_else(|| "naturalna, środkowa".to_owned(), |v| v.timbre.clone());
                check_text(&timbre)?;
                let tempo = if preset.rate < 0.95 {
                    "wolne"
                } else if preset.rate > 1.05 {
                    "żwawe"
                } else {
                    "naturalne"
                };
                let design = draft.map_or_else(
                    || {
                        format!(
                            "młoda dorosła kobieta, ok. {age} lat, {timbre}, czysta polszczyzna"
                        )
                    },
                    |v| v.design_prompt.clone(),
                );
                check_text(&design)?;
                let bible = VoiceBible {
                    language: "polski".into(),
                    perceived_age: age,
                    timbre,
                    register: "środkowy".into(),
                    tempo: tempo.into(),
                    energy: "spokojna".into(),
                    emotion_range: "naturalny".into(),
                    design_prompt: design,
                    provenance: "głos v0 wbudowany (Kreator agentów); brak prawdziwej osoby".into(),
                };
                return Ok((preset, bible));
            }
            Err(e) => last = BuildError::Conflict(format!("głos: {e}")),
        }
    }
    Err(last)
}

fn role(
    draft: &AgentDraft,
    policy: &BuilderPolicy,
    warnings: &mut Vec<String>,
) -> Result<Role, BuildError> {
    let r = draft
        .role
        .as_ref()
        .ok_or_else(|| BuildError::Missing("rola".into()))?;
    if !is_valid_id(&r.id) {
        return Err(BuildError::Invalid("identyfikator roli".into()));
    }
    check_kernel_id(&r.id)?;
    let name = field("nazwa roli", &Some(r.name.clone()), 64)?;
    check_kernel_id(&fold(&name).replace(' ', "-"))?;
    let description = field("opis roli", &Some(r.description.clone()), 400)?;
    let prompt = field("prompt roli", &Some(r.prompt.clone()), 4000)?;
    check_feminine(&prompt)?;
    if !MODEL_POLICIES.contains(&r.model_policy.as_str()) {
        return Err(BuildError::Invalid("klasa modelu roli".into()));
    }
    for g in &r.tools {
        policy.check_group(g)?;
    }
    let mut untrusted = r.untrusted_isolated;
    if r.tools
        .iter()
        .any(|g| matches!(g.as_str(), "web" | "browser" | "mcp"))
        && !untrusted
    {
        untrusted = true;
        warnings.push(
            "rola z dostępem do źródeł zewnętrznych pracuje w izolacji (treść niezaufana)".into(),
        );
    }
    if r.read_only
        && r.tools
            .iter()
            .any(|g| !matches!(g.as_str(), "fs.read" | "web" | "browser" | "mcp" | "memory"))
    {
        warnings
            .push("rola tylko do odczytu — narzędzia zmieniające stan nie będą dostępne".into());
    }
    Ok(Role {
        id: RoleId::new(r.id.clone()),
        name,
        description,
        prompt,
        model_policy: r.model_policy.clone(),
        tools: r.tools.clone(),
        read_only: r.read_only,
        untrusted_isolated: untrusted,
        author: r.author,
        unique: false,
        builtin: false,
    })
}

fn limits(draft: &AgentDraft, policy: &BuilderPolicy) -> Result<AgentLimits, BuildError> {
    let l = &draft.limits;
    let autonomy = l.autonomy.unwrap_or(policy.ceiling.min(AutonomyLevel::L3));
    policy.check_autonomy(autonomy)?;
    let budget = l.budget.unwrap_or_default();
    if budget.max_steps == 0
        || budget.max_tokens == 0
        || budget.max_wall_ms == 0
        || budget.max_tool_calls_per_turn == 0
    {
        return Err(BuildError::Invalid("budżet".into()));
    }
    if !agent_runtime_contract::budget_within(&budget, &policy.max_budget) {
        return Err(BuildError::BudgetTooHigh);
    }
    for s in &l.fs_write {
        check_fs_scope(s)?;
    }
    let memory_scope = l.memory_scope.clone().unwrap_or_else(|| "agent".into());
    if !matches!(memory_scope.as_str(), "agent" | "session") {
        return Err(BuildError::Invalid(
            "zakres pamięci (agent albo session)".into(),
        ));
    }
    let retain_days = l.retain_days.unwrap_or(30);
    if !(1..=365).contains(&retain_days) {
        return Err(BuildError::Invalid("retencja pamięci (1–365 dni)".into()));
    }
    for t in &l.triggers {
        let fields: Vec<&str> = t.split_whitespace().collect();
        if fields.len() != 5
            || fields
                .iter()
                .any(|f| !f.chars().all(|c| c.is_ascii_digit() || "*/,-".contains(c)))
        {
            return Err(BuildError::Invalid(format!("wyzwalacz `{t}`")));
        }
    }
    Ok(AgentLimits {
        autonomy,
        budget,
        fs_write: l.fs_write.clone(),
        memory_scope,
        retain_days,
        triggers: l.triggers.clone(),
    })
}

/// Buduje manifest ze szkicu (całość albo błąd; ostrzeżenia dla właściciela).
pub fn build_manifest(
    draft: &AgentDraft,
    policy: &BuilderPolicy,
    ctx: &BuildContext<'_>,
) -> Result<Built, BuildError> {
    let mut warnings = Vec::new();
    let name = field("imię", &draft.name, 32)?;
    let mut letters = name.chars();
    if !letters.next().is_some_and(char::is_uppercase) || !name.chars().all(char::is_alphabetic) {
        return Err(BuildError::Invalid("imię (litery, wielka pierwsza)".into()));
    }
    let id = persona_id(&name, draft.id.as_deref())?;
    let forms = draft
        .forms
        .clone()
        .unwrap_or_else(|| decline_feminine(&name));
    let character = field("charakter", &draft.character, 200)?;
    let color = match &draft.color {
        Some(c) if policy.palette.contains(c) => c.clone(),
        Some(_) => return Err(BuildError::Invalid("kolor spoza palety".into())),
        None => policy
            .palette
            .iter()
            .find(|c| {
                !ctx.catalog
                    .personas()
                    .iter()
                    .any(|p| p.color.as_str() == c.as_str())
            })
            .cloned()
            .ok_or_else(|| BuildError::Conflict("brak wolnego koloru w palecie".into()))?,
    };
    let (preset, bible) = voice(draft.voice.as_ref(), &id, ctx)?;
    let persona = Persona {
        id: id.clone(),
        name: name.clone(),
        glyph: draft
            .glyph
            .unwrap_or_else(|| name.chars().next().unwrap_or('?')),
        color: ColorToken::new(color),
        character,
        wake_phrases: vec![format!("Hej {name}")],
        forms,
        voice: bible,
        builtin: false,
    };
    let role = role(draft, policy, &mut warnings)?;
    let limits = limits(draft, policy)?;
    let skill_ok = |s: &String| {
        s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && (2..=64).contains(&s.len())
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    if draft.skills.len() > 16 || !draft.skills.iter().all(skill_ok) {
        return Err(BuildError::Invalid("umiejętności".into()));
    }
    let mut probe = ctx.catalog.clone();
    probe
        .add_role(role.clone())
        .map_err(|e| BuildError::Conflict(e.to_string()))?;
    probe
        .add_persona(persona.clone())
        .map_err(|e| BuildError::Conflict(e.to_string()))?;
    let prompt = render_system_prompt(DEFAULT_PROMPT_TEMPLATE, &persona, &[&role], &[])
        .map_err(|e| BuildError::Invalid(e.to_string()))?;
    check_feminine(&prompt)?;
    if !limits.triggers.is_empty() {
        warnings.push("wyzwalacze wymagają osobnego zatwierdzenia w module wyzwalaczy".into());
    }
    let manifest = AgentManifest {
        persona,
        role,
        voice: preset,
        limits,
        skills: draft.skills.clone(),
    };
    let hash = manifest_hash(&manifest)?;
    Ok(Built {
        manifest,
        hash,
        warnings,
    })
}
