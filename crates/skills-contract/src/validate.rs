//! Walidacja przepisu (pola, schemat, narzędzia i zdolności zgodne z manifestami, zakaz
//! zdolności Jądra), testy akceptacyjne, skaner treści (podejrzane polecenia → kwarantanna)
//! i złożenie celu przebiegu z szablonu, parametrów i kroków.

use std::collections::BTreeSet;

use personas_contract::fold;
use tools_common_contract::ToolManifest;

use crate::error::SkillError;
use crate::model::Skill;
use crate::schema::{placeholders, render, validate_params, validate_schema};

/// Rodziny zdolności, których umiejętność nie może wymagać (Jądro, sekrety).
pub const FORBIDDEN_CAPABILITIES: [&str; 2] = ["system.admin", "secrets.read"];

/// Fragmenty (po `fold`) wskazujące na próbę obejścia zabezpieczeń albo wstrzyknięcie.
const SUSPICIOUS: [&str; 22] = [
    "zignoruj",
    "ignore previous",
    "ignore all",
    "<<<niezaufane",
    "poziom autonomii",
    "podnies uprawnienia",
    "broker",
    "jadro",
    "jadra",
    "kill-switch",
    "wylacz audyt",
    "deny-list",
    "~/.claude",
    ".codex",
    "cookies",
    "ciasteczk",
    "credential",
    "system.admin",
    "secrets.read",
    "powershell -enc",
    "invoke-webrequest",
    "curl http",
];

fn len_ok(s: &str, min: usize, max: usize) -> bool {
    let n = s.trim().chars().count();
    (min..=max).contains(&n)
}

fn invalid(what: &str) -> SkillError {
    SkillError::Invalid(what.to_owned())
}

/// Pełna walidacja przepisu względem katalogu narzędzi.
pub fn validate_skill(skill: &Skill, catalog: &[ToolManifest]) -> Result<(), SkillError> {
    if !skill.id.is_valid() {
        return Err(invalid("identyfikator spoza [a-z][a-z0-9-]{1,63}"));
    }
    if !len_ok(&skill.name, 1, 80) || !len_ok(&skill.description, 10, 1000) {
        return Err(invalid("nazwa (1–80) albo opis (10–1000 znaków)"));
    }
    if !len_ok(&skill.prompt, 1, 4000) {
        return Err(invalid("szablon polecenia (1–4000 znaków)"));
    }
    if skill.keywords.len() > 16 || skill.keywords.iter().any(|k| !len_ok(k, 1, 40)) {
        return Err(invalid("słowa kluczowe (≤ 16, każde 1–40 znaków)"));
    }
    if skill.steps.len() > 30 || skill.steps.iter().any(|s| !len_ok(s, 1, 500)) {
        return Err(invalid("kroki (≤ 30, każdy 1–500 znaków)"));
    }
    if skill.examples.len() > 10 || skill.acceptance.is_empty() || skill.acceptance.len() > 20 {
        return Err(invalid("przykłady ≤ 10, testy akceptacyjne 1–20"));
    }
    if let Some(b) = &skill.budget
        && (b.max_steps == 0
            || b.max_tokens == 0
            || b.max_wall_ms == 0
            || b.max_tool_calls_per_turn == 0)
    {
        return Err(invalid("budżet musi być dodatni"));
    }
    validate_schema(&skill.parameters).map_err(SkillError::Params)?;
    let declared: BTreeSet<String> = skill
        .parameters
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    for p in placeholders(&skill.prompt).map_err(SkillError::Params)? {
        if !declared.contains(&p) {
            return Err(SkillError::Params(format!(
                "szablon używa niezadeklarowanego `{p}`"
            )));
        }
    }
    check_tools(skill, catalog)
}

/// Narzędzia istnieją, bez powtórzeń; zdolności zadeklarowane = zdolności z manifestów; bez Jądra.
pub fn check_tools(skill: &Skill, catalog: &[ToolManifest]) -> Result<(), SkillError> {
    if let Some(bad) = skill
        .required_capabilities
        .iter()
        .find(|c| FORBIDDEN_CAPABILITIES.contains(&c.as_str()))
    {
        return Err(SkillError::ForbiddenCapability(bad.clone()));
    }
    let unique: BTreeSet<&String> = skill.required_tools.iter().collect();
    if unique.len() != skill.required_tools.len() || unique.len() > 16 {
        return Err(invalid("narzędzia bez powtórzeń, ≤ 16"));
    }
    let mut actual = BTreeSet::new();
    for t in &skill.required_tools {
        let m = catalog
            .iter()
            .find(|m| &m.name == t)
            .ok_or_else(|| SkillError::UnknownTool(t.clone()))?;
        actual.extend(m.capabilities.iter().cloned());
    }
    if let Some(bad) = actual
        .iter()
        .find(|c| FORBIDDEN_CAPABILITIES.contains(&c.as_str()))
    {
        return Err(SkillError::ForbiddenCapability(bad.clone()));
    }
    let declared: BTreeSet<String> = skill.required_capabilities.iter().cloned().collect();
    if declared != actual {
        return Err(SkillError::CapabilityMismatch {
            declared: declared.into_iter().collect(),
            actual: actual.into_iter().collect(),
        });
    }
    Ok(())
}

/// Cel przebiegu: szablon z parametrami + kroki.
pub fn render_goal(skill: &Skill, params: &serde_json::Value) -> Result<String, SkillError> {
    let p = validate_params(&skill.parameters, params).map_err(SkillError::Params)?;
    let mut goal = render(&skill.prompt, &p).map_err(SkillError::Params)?;
    if !skill.steps.is_empty() {
        goal.push_str("\n\nKroki umiejętności „");
        goal.push_str(&skill.name);
        goal.push_str("”:");
        for (i, s) in skill.steps.iter().enumerate() {
            goal.push_str(&format!("\n{}. {}", i + 1, s.trim()));
        }
    }
    Ok(goal)
}

/// Testy akceptacyjne przepisu (deterministyczne, bez modelu).
pub fn run_acceptance(skill: &Skill) -> Result<(), SkillError> {
    for t in &skill.acceptance {
        let fail = |reason: String| SkillError::Acceptance {
            test: t.name.clone(),
            reason,
        };
        match (render_goal(skill, &t.params), t.expect_rejected) {
            (Ok(_), true) => return Err(fail("parametry miały zostać odrzucone".into())),
            (Err(_), true) => {}
            (Err(e), false) => return Err(fail(e.to_string())),
            (Ok(goal), false) => {
                if let Some(missing) = t.expect_in_goal.iter().find(|x| !goal.contains(x.as_str()))
                {
                    return Err(fail(format!("cel nie zawiera „{missing}”")));
                }
            }
        }
    }
    Ok(())
}

/// Znaki niewidoczne (formatujące), którymi można rozbić frazę: miękki dywiz, spacje zerowej
/// szerokości, łączniki, znaczniki kierunku, BOM.
fn invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
    )
}

/// Napisy z wartości JSON (klucze i wartości; bez ucieczek `\\n` w miejscu znaków).
fn json_strings(v: &serde_json::Value, out: &mut String) {
    match v {
        serde_json::Value::String(s) => {
            out.push(' ');
            out.push_str(s);
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| json_strings(x, out)),
        serde_json::Value::Object(m) => {
            for (k, x) in m {
                out.push(' ');
                out.push_str(k);
                json_strings(x, out);
            }
        }
        _ => {}
    }
}

/// Postać do skanowania: `fold`, bez znaków niewidocznych, białe znaki zwinięte do spacji.
fn scan_form(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in fold(text).chars().filter(|c| !invisible(*c)) {
        if c.is_whitespace() {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Skaner treści: podejrzane fragmenty (powody dla właściciela; z niezaufanego źródła →
/// kwarantanna). Skanowane jest wszystko, co trafia do celu przebiegu albo do przeglądu:
/// także schemat parametrów (wartości domyślne i `enum` są wstawiane do celu), przykłady
/// i testy akceptacyjne (przegląd #2, SR2-02).
pub fn scan(skill: &Skill) -> Vec<String> {
    let mut text = format!("{} {} {}", skill.name, skill.description, skill.prompt);
    for s in skill.steps.iter().chain(&skill.keywords) {
        text.push(' ');
        text.push_str(s);
    }
    json_strings(&skill.parameters, &mut text);
    for e in &skill.examples {
        text.push(' ');
        text.push_str(&e.request);
        json_strings(&e.params, &mut text);
    }
    for t in &skill.acceptance {
        text.push(' ');
        text.push_str(&t.name);
        json_strings(&t.params, &mut text);
        for x in &t.expect_in_goal {
            text.push(' ');
            text.push_str(x);
        }
    }
    let folded = scan_form(&text);
    SUSPICIOUS
        .iter()
        .filter(|p| folded.contains(*p))
        .map(|p| format!("podejrzany fragment: „{p}”"))
        .collect()
}
