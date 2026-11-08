//! Rozwiązywanie agentki i ról delegacji.
use crate::engine::ChatEngine;
use agent_runtime_contract::DELEGATE_GROUP;
use personas_contract::{Persona, Role};
use sessions_contract::SessionId;
/// Rozwiązuje docelową agentkę, jej role w obsadzie oraz cel zadania z JSON argumentów.
pub(crate) fn resolve_delegation(
    core: &ChatEngine,
    session: &SessionId,
    parent_agent: &str,
    input: &serde_json::Value,
) -> Result<(Persona, Vec<Role>, String), String> {
    let goal = input
        .get("goal")
        .or_else(|| input.get("task"))
        .or_else(|| input.get("zadanie"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if goal.is_empty() {
        return Err("Pusty cel podzadania (`goal`).".into());
    }
    let raw_role = input
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim();
    let raw_persona = input
        .get("persona")
        .or_else(|| input.get("agent"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim();
    let cast = core.inner.personas.cast(session);
    let personas = core.inner.personas.personas();
    let all_roles = core.inner.personas.roles();

    let match_persona = |s: &str| -> Option<Persona> {
        if s.is_empty() {
            return None;
        }
        personas
            .iter()
            .find(|p| {
                cast.personas().contains(&p.id)
                    && (p.id.as_str().eq_ignore_ascii_case(s)
                        || p.name.eq_ignore_ascii_case(s)
                        || p.forms.accusative.eq_ignore_ascii_case(s)
                        || p.forms.dative.eq_ignore_ascii_case(s))
            })
            .cloned()
    };
    let match_role = |s: &str| -> Option<Role> {
        if s.is_empty() {
            return None;
        }
        all_roles
            .iter()
            .find(|r| {
                r.id.as_str().eq_ignore_ascii_case(s)
                    || r.name.eq_ignore_ascii_case(s)
                    || r.name
                        .split('/')
                        .any(|part| part.trim().eq_ignore_ascii_case(s))
            })
            .cloned()
    };

    let wanted_role = match_role(raw_role);
    let persona = match_persona(raw_persona)
        .or_else(|| match_persona(raw_role))
        .or_else(|| {
            let r = wanted_role.as_ref()?;
            let holders = cast.holders(&r.id);
            let chosen = holders
                .iter()
                .find(|h| h.as_str() != parent_agent)
                .or_else(|| holders.first())?;
            personas.iter().find(|p| &p.id == chosen).cloned()
        })
        .or_else(|| {
            personas
                .into_iter()
                .find(|p| cast.personas().contains(&p.id) && p.id.as_str() != parent_agent)
        })
        .ok_or_else(|| {
            format!("Brak pasującej agentki w obsadzie dla `{raw_role}` / `{raw_persona}`.")
        })?;

    let mut roles = core.cast_roles(session, &persona.id);
    roles.retain(|r| !r.tools.iter().any(|g| g == DELEGATE_GROUP));
    if let Some(primary) = wanted_role {
        if let Some(pos) = roles.iter().position(|r| r.id == primary.id) {
            let r = roles.remove(pos);
            roles.insert(0, r);
        } else {
            roles.insert(0, primary);
        }
    }
    if roles.is_empty() {
        return Err(format!(
            "Agentka {} nie ma przydzielonych ról wykonawczych.",
            persona.name
        ));
    }
    Ok((persona, roles, goal))
}
