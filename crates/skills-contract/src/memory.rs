//! Szkic umiejętności z pamięci proceduralnej (konsolidacja Strażniczki pamięci, PLAN §10,
//! §12.4): tytuł z pierwszej linii, kroki z listy, narzędzia z nazw wspomnianych w treści
//! (tylko z katalogu), źródło z proweniencją wpisu (niezaufany → kwarantanna przy propozycji).
//! Szkic zawsze przechodzi zwykłą ścieżkę: propozycja → przegląd → zatwierdzenie właściciela.

use std::collections::BTreeSet;

use memory_contract::{EntryRef, Layer, MemoryEntry};
use personas_contract::fold;
use semver::Version;
use tools_common_contract::ToolManifest;

use crate::error::SkillError;
use crate::model::{AcceptanceTest, Skill, SkillId, SkillSource};

fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in fold(text).chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
        if out.len() >= 40 {
            break;
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    format!("pamiec-{out}")
}

fn steps_of(body: &str) -> Vec<String> {
    let lines: Vec<String> = body
        .lines()
        .map(|l| {
            l.trim()
                .trim_start_matches(|c: char| {
                    c.is_ascii_digit() || matches!(c, '.' | ')' | '-' | '*' | '•')
                })
                .trim()
                .to_owned()
        })
        .filter(|l| !l.is_empty())
        .collect();
    let items = if lines.len() > 1 {
        lines
    } else {
        body.split([';', ','])
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    };
    items
        .into_iter()
        .take(30)
        .map(|s| s.chars().take(500).collect())
        .collect()
}

/// Szkic z wpisu warstwy proceduralnej.
pub fn draft_from_memory(
    entry: &MemoryEntry,
    catalog: &[ToolManifest],
) -> Result<(Skill, SkillSource), SkillError> {
    if entry.layer != Layer::Procedural {
        return Err(SkillError::Invalid(
            "wpis spoza warstwy proceduralnej".into(),
        ));
    }
    let text = entry.text.trim();
    let (title, body) = match text.split_once(['\n', ':']) {
        Some((t, b)) if !b.trim().is_empty() => (t.trim(), b.trim()),
        _ => (text, text),
    };
    let name: String = title.chars().take(80).collect();
    if name.trim().is_empty() {
        return Err(SkillError::Invalid("pusty wpis".into()));
    }
    let folded = fold(text);
    let tools: Vec<&ToolManifest> = catalog
        .iter()
        .filter(|m| folded.contains(&fold(&m.name)))
        .collect();
    let capabilities: BTreeSet<String> = tools
        .iter()
        .flat_map(|m| m.capabilities.iter().cloned())
        .collect();
    let skill = Skill {
        id: SkillId::new(slug(&name)),
        version: Version::new(0, 1, 0),
        name: name.clone(),
        description: format!(
            "Procedura wyuczona z pamięci: {}",
            name.chars().take(200).collect::<String>()
        ),
        keywords: Vec::new(),
        required_tools: tools.iter().map(|m| m.name.clone()).collect(),
        required_capabilities: capabilities.into_iter().collect(),
        parameters: serde_json::json!({"type": "object", "properties": {}, "additionalProperties": false}),
        prompt: format!("Wykonaj procedurę „{name}”."),
        steps: steps_of(body),
        examples: Vec::new(),
        acceptance: vec![AcceptanceTest {
            name: "cel zawiera nazwę".into(),
            params: serde_json::Value::Null,
            expect_in_goal: vec![name],
            expect_rejected: false,
        }],
        budget: None,
    };
    let source = SkillSource::Memory {
        entry: EntryRef::new(entry.scope.clone(), entry.id.clone()).to_string(),
        trusted: entry.trusted,
    };
    Ok((skill, source))
}
