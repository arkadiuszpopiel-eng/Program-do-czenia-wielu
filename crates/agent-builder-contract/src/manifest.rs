//! Hash manifestu (kanoniczny JSON — klucze posortowane — SHA-256 hex) i szkic odtworzony
//! z manifestu (ponowna walidacja przed testem na sucho i zapisem).

use sha2::{Digest, Sha256};

use crate::draft::{AgentDraft, AgentManifest, LimitsDraft, RoleDraft, VoiceDraft};
use crate::policy::BuildError;

fn canonical(v: &serde_json::Value, out: &mut String) {
    match v {
        serde_json::Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::Value::String(k.clone()).to_string());
                out.push(':');
                if let Some(x) = m.get(k) {
                    canonical(x, out);
                }
            }
            out.push('}');
        }
        serde_json::Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(x, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// Hash manifestu (kanoniczny JSON, SHA-256 hex).
pub fn manifest_hash(m: &AgentManifest) -> Result<String, BuildError> {
    let v = serde_json::to_value(m).map_err(|e| BuildError::Invalid(e.to_string()))?;
    let mut s = String::new();
    canonical(&v, &mut s);
    Ok(Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Szkic odtworzony z manifestu (ponowna walidacja przed zapisem — manifest ma pola publiczne).
pub fn draft_of(m: &AgentManifest) -> AgentDraft {
    AgentDraft {
        id: Some(m.persona.id.to_string()),
        name: Some(m.persona.name.clone()),
        forms: Some(m.persona.forms.clone()),
        glyph: Some(m.persona.glyph),
        color: Some(m.persona.color.to_string()),
        character: Some(m.persona.character.clone()),
        voice: Some(VoiceDraft {
            base: m.voice.base_speaker.clone(),
            pitch: m.voice.pitch,
            rate: m.voice.rate,
            perceived_age: m.persona.voice.perceived_age,
            timbre: m.persona.voice.timbre.clone(),
            design_prompt: m.persona.voice.design_prompt.clone(),
        }),
        role: Some(RoleDraft {
            id: m.role.id.to_string(),
            name: m.role.name.clone(),
            description: m.role.description.clone(),
            prompt: m.role.prompt.clone(),
            model_policy: m.role.model_policy.clone(),
            tools: m.role.tools.clone(),
            read_only: m.role.read_only,
            untrusted_isolated: m.role.untrusted_isolated,
            author: m.role.author,
        }),
        limits: LimitsDraft {
            autonomy: Some(m.limits.autonomy),
            budget: Some(m.limits.budget),
            fs_write: m.limits.fs_write.clone(),
            memory_scope: Some(m.limits.memory_scope.clone()),
            retain_days: Some(m.limits.retain_days),
            triggers: m.limits.triggers.clone(),
        },
        skills: m.skills.clone(),
    }
}
