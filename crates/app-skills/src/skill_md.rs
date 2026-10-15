//! Import umiejętności w formacie Agent Skills (`SKILL.md`, C2.6): nagłówek YAML (`name`,
//! `description`, opcjonalnie `version`) i treść Markdown → przepis Alfy w paczce
//! `alfa.skills.v1`. Paczka idzie zwykłą drogą importu z zewnątrz: kwarantanna i zatwierdzenie
//! właściciela z hashem przejrzanej treści. Narzędzia z `allowed-tools` innych programów nie są
//! przenoszone — uprawnienia nadaje się w Alfie. Treść: początek w szablonie polecenia, reszta
//! w krokach (limity przepisu); `{{…}}` z treści jest rozbrajane, żeby nie udawało parametrów.

use semver::Version;
use skills_contract::{
    AcceptanceTest, BundledSkill, ImportOrigin, Skill, SkillBundle, SkillId, SkillSource,
};

/// Część treści w szablonie polecenia (limit przepisu 4000 znaków, z miejscem na zadanie).
const PROMPT_BODY: usize = 3600;
/// Długość kroku (limit przepisu 500 znaków).
const STEP: usize = 480;
/// Najwięcej kroków (limit przepisu 30).
const MAX_STEPS: usize = 30;

/// Czy plik wygląda na `SKILL.md` (nagłówek YAML na początku).
pub fn is_skill_md(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(64)]);
    text.trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with("---")
}

/// Nagłówek YAML: proste pary `klucz: wartość` i bloki `>` / `|` z wcięciem.
fn front_matter(text: &str) -> Result<(Vec<(String, String)>, String), String> {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let rest = text.strip_prefix("---").ok_or("brak nagłówka YAML (---)")?;
    let end = rest.find("\n---").ok_or("niedomknięty nagłówek YAML")?;
    let (head, body) = (&rest[..end], &rest[end + 4..]);
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut lines = head.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim().is_empty() || line.starts_with(' ') || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let mut value = value.trim().to_owned();
        if value == ">" || value == "|" || value == ">-" || value == "|-" {
            let folded = value.starts_with('>');
            let mut parts: Vec<String> = Vec::new();
            while let Some(next) = lines.peek() {
                if !next.starts_with(' ') && !next.trim().is_empty() {
                    break;
                }
                parts.push(next.trim().to_owned());
                lines.next();
            }
            value = parts
                .join(if folded { " " } else { "\n" })
                .trim()
                .to_owned();
        }
        let value = value.trim_matches(['"', '\'']).to_owned();
        pairs.push((key.trim().to_lowercase(), value));
    }
    Ok((pairs, body.trim_start_matches(['\r', '\n']).to_owned()))
}

/// Identyfikator przepisu z nazwy (`[a-z][a-z0-9-]{1,63}`).
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        let c = match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            other => other,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let mut id = out
        .trim_end_matches('-')
        .chars()
        .take(60)
        .collect::<String>();
    id = id.trim_end_matches('-').to_owned();
    if !id.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        id = format!("skill-{id}");
    }
    if id.len() < 2 {
        id = "skill-md".into();
    }
    id
}

/// Fragmenty tekstu po `max` znaków (na granicach znaków).
fn chunks(text: &str, max: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(max)
        .map(|c| c.iter().collect::<String>().trim().to_owned())
        .filter(|c| !c.is_empty())
        .collect()
}

/// Paczka z jednym przepisem z pliku `SKILL.md`.
pub fn bundle(text: &str) -> Result<SkillBundle, String> {
    let (pairs, body) = front_matter(text)?;
    let get = |k: &str| {
        pairs
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone())
    };
    let name = get("name")
        .filter(|n| !n.is_empty())
        .ok_or("brak pola `name`")?;
    let description = get("description").unwrap_or_default();
    if description.chars().count() < 10 {
        return Err("pole `description` za krótkie (co najmniej 10 znaków)".into());
    }
    let version = get("version")
        .and_then(|v| Version::parse(&v).ok())
        .unwrap_or_else(|| Version::new(1, 0, 0));
    // `{{` z treści nie może udawać parametru szablonu.
    let body = body.replace("{{", "{ {").replace("}}", "} }");
    let body: String = body.trim().to_owned();
    let (head, tail) = match body.char_indices().nth(PROMPT_BODY) {
        Some((cut, _)) => (body[..cut].to_owned(), body[cut..].to_owned()),
        None => (body.clone(), String::new()),
    };
    let steps = chunks(&tail, STEP);
    if steps.len() > MAX_STEPS {
        return Err("treść SKILL.md za długa (najwyżej ok. 18 000 znaków)".into());
    }
    let keywords: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2)
        .map(|w| w.chars().take(40).collect::<String>().to_lowercase())
        .take(16)
        .collect();
    let skill = Skill {
        id: SkillId::new(slug(&name)),
        version,
        name: name.chars().take(80).collect(),
        description: description.chars().take(1000).collect(),
        keywords,
        required_tools: Vec::new(),
        required_capabilities: Vec::new(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "zadanie": { "type": "string", "description": "Czego dotyczy prośba" }
            },
            "required": ["zadanie"],
            "additionalProperties": false
        }),
        prompt: format!("{head}\n\nZadanie: {{{{zadanie}}}}"),
        steps,
        examples: Vec::new(),
        acceptance: vec![AcceptanceTest {
            name: "zadanie trafia do celu".into(),
            params: serde_json::json!({ "zadanie": "próba importu" }),
            expect_in_goal: vec!["próba importu".into()],
            expect_rejected: false,
        }],
        budget: None,
    };
    SkillBundle::new(vec![BundledSkill {
        skill,
        source: SkillSource::Import {
            origin: ImportOrigin::External,
        },
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\nname: Raport z PDF\ndescription: >\n  Wyciąga tabele z raportów PDF\n  i podsumowuje liczby.\nallowed-tools: Read, Bash\n---\n# Raport z PDF\n\n## Instrukcje\n\n1. Otwórz plik.\n2. Znajdź tabele {{nie-parametr}}.\n";

    #[test]
    fn skill_md_becomes_valid_quarantined_bundle() {
        assert!(is_skill_md(SAMPLE.as_bytes()));
        assert!(!is_skill_md(b"{\"format\":\"alfa.skills.v1\"}"));
        let b = bundle(SAMPLE).unwrap();
        b.verify().unwrap();
        let s = &b.skills[0].skill;
        assert_eq!(s.id.as_str(), "raport-z-pdf");
        assert_eq!(
            s.description,
            "Wyciąga tabele z raportów PDF i podsumowuje liczby."
        );
        assert!(
            s.prompt.contains("Znajdź tabele { {nie-parametr} }"),
            "{}",
            s.prompt
        );
        assert!(s.prompt.ends_with("Zadanie: {{zadanie}}"));
        assert!(b.skills[0].source.is_untrusted(), "kwarantanna");
        skills_contract::validate_skill(s, &[]).unwrap();
    }

    #[test]
    fn long_bodies_go_to_steps_and_bad_files_are_refused() {
        let long = format!(
            "---\nname: Długa\ndescription: Bardzo długi przepis do kroków\n---\n{}",
            "abc ".repeat(2000)
        );
        let b = bundle(&long).unwrap();
        let s = &b.skills[0].skill;
        assert!(!s.steps.is_empty() && s.steps.len() <= MAX_STEPS);
        skills_contract::validate_skill(s, &[]).unwrap();
        assert!(bundle("bez nagłówka").is_err());
        assert!(bundle("---\nname: x\n").is_err(), "niedomknięty");
        assert!(bundle("---\nname: Krótki\ndescription: za mało\n---\nTreść").is_err());
        let huge = format!(
            "---\nname: Wielka\ndescription: Za długi przepis do importu\n---\n{}",
            "x".repeat(30_000)
        );
        assert!(bundle(&huge).is_err());
        assert_eq!(slug("123 Test"), "skill-123-test");
    }
}
