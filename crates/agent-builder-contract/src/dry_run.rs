//! Podgląd i test na sucho (PLAN §9.5 „test w piaskownicy”): skryptowany przebieg bez modelu
//! i bez skutków — każdy krok sprawdzany jak w runtime (narzędzie dozwolone dla roli, zakres
//! zapisu, poziom autonomii → sama / zapyta / odmowa) i porównany z oczekiwaniem scenariusza.

use personas_contract::{DEFAULT_PROMPT_TEMPLATE, render_system_prompt};
use risk_classifier_contract::AutonomyLevel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::ToolManifest;

use crate::draft::{AgentManifest, DryExpect, DryScenario};

/// Podgląd manifestu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Preview {
    /// Prompt systemowy (rodzaj żeński).
    pub system_prompt: String,
    /// Narzędzia dostępne dla roli (z katalogu).
    pub tools: Vec<String>,
    /// Głos (silnik bazowy, wysokość, tempo).
    pub voice: String,
    /// Poziom autonomii.
    pub autonomy: AutonomyLevel,
}

/// Wynik kroku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DryStepResult {
    /// Narzędzie.
    pub tool: String,
    /// Co by się stało.
    pub outcome: DryExpect,
    /// Oczekiwane.
    pub expected: DryExpect,
    /// Wyjaśnienie (PL).
    pub why: String,
}

/// Raport testu na sucho.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DryRunReport {
    /// Podgląd.
    pub preview: Preview,
    /// Kroki.
    pub steps: Vec<DryStepResult>,
    /// Wszystkie kroki zgodne z oczekiwaniem (i scenariusz niepusty).
    pub passed: bool,
}

/// Podgląd: prompt, narzędzia roli, głos, autonomia.
pub fn preview(m: &AgentManifest, catalog: &[ToolManifest]) -> Preview {
    let system_prompt = render_system_prompt(DEFAULT_PROMPT_TEMPLATE, &m.persona, &[&m.role], &[])
        .unwrap_or_default();
    let tools = catalog
        .iter()
        .filter(|t| t.allowed_for(&m.role.tools, m.role.read_only))
        .map(|t| t.name.clone())
        .collect();
    Preview {
        system_prompt,
        tools,
        voice: format!(
            "{} · wysokość {:.2} · tempo {:.2}",
            m.voice.base_speaker, m.voice.pitch, m.voice.rate
        ),
        autonomy: m.limits.autonomy,
    }
}

fn norm(p: &str) -> String {
    p.trim().replace('\\', "/").to_lowercase()
}

/// Czy ścieżka mieści się w zakresie (`prefiks/**` albo dokładnie).
pub fn in_scope(path: &str, scope: &str) -> bool {
    let (p, s) = (norm(path), norm(scope));
    if p.contains("..") {
        return false;
    }
    match s.strip_suffix("/**") {
        Some(prefix) => p == prefix || p.starts_with(&format!("{prefix}/")),
        None => p == s,
    }
}

fn step(
    m: &AgentManifest,
    catalog: &[ToolManifest],
    tool: &str,
    args: &serde_json::Value,
) -> (DryExpect, String) {
    let Some(t) = catalog.iter().find(|t| t.name == tool) else {
        return (DryExpect::Denied, "nieznane narzędzie".into());
    };
    if !t.allowed_for(&m.role.tools, m.role.read_only) {
        return (DryExpect::Denied, "rola nie ma tego narzędzia".into());
    }
    if !t.mutating {
        return (DryExpect::Allowed, "odczyt".into());
    }
    if t.capabilities.iter().any(|c| c == "fs.write") {
        let paths: Vec<&str> = ["path", "from", "to"]
            .iter()
            .filter_map(|k| args.get(*k).and_then(serde_json::Value::as_str))
            .collect();
        if paths.is_empty()
            || !paths
                .iter()
                .all(|p| m.limits.fs_write.iter().any(|s| in_scope(p, s)))
        {
            return (DryExpect::Denied, "zapis poza zakresem agentki".into());
        }
    }
    let destructive = t.reversible != risk_classifier_contract::Reversibility::Yes;
    match m.limits.autonomy {
        AutonomyLevel::L0 => (DryExpect::Denied, "poziom L0: tylko podgląd".into()),
        AutonomyLevel::L1 => (DryExpect::Ask, "poziom L1: pyta o każdą zmianę".into()),
        AutonomyLevel::L2 if destructive => (DryExpect::Ask, "poziom L2: pyta o ryzykowne".into()),
        _ => (DryExpect::Allowed, "w zakresie i poziomie autonomii".into()),
    }
}

/// Test na sucho: każdy krok oceniony bez wykonania; zaliczony, gdy wszystkie zgodne.
pub fn dry_run(
    m: &AgentManifest,
    catalog: &[ToolManifest],
    scenario: &DryScenario,
) -> DryRunReport {
    let steps: Vec<DryStepResult> = scenario
        .steps
        .iter()
        .map(|s| {
            let (outcome, why) = step(m, catalog, &s.tool, &s.args);
            DryStepResult {
                tool: s.tool.clone(),
                outcome,
                expected: s.expect,
                why,
            }
        })
        .collect();
    let passed = !steps.is_empty() && steps.iter().all(|s| s.outcome == s.expected);
    DryRunReport {
        preview: preview(m, catalog),
        steps,
        passed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes() {
        assert!(in_scope(
            "%USERPROFILE%\\Downloads\\a.pdf",
            "%USERPROFILE%\\Downloads\\**"
        ));
        assert!(in_scope(
            "%userprofile%/downloads",
            "%USERPROFILE%/Downloads/**"
        ));
        assert!(!in_scope(
            "%USERPROFILE%\\Downloads\\..\\.ssh\\id",
            "%USERPROFILE%\\Downloads\\**"
        ));
        assert!(!in_scope(
            "%USERPROFILE%\\Documents\\a",
            "%USERPROFILE%\\Downloads\\**"
        ));
        assert!(in_scope("~/a.txt", "~/a.txt") && !in_scope("~/b.txt", "~/a.txt"));
    }
}
