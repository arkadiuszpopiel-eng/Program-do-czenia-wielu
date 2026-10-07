//! Dane przykładowe (katalog narzędzi, umiejętność „porządki w Pobranych”) — testy
//! kontraktowe, atrapa, dokumentacja.

use risk_classifier_contract::Reversibility;
use semver::Version;
use serde_json::json;
use tools_common_contract::ToolManifest;

use crate::model::{AcceptanceTest, Skill, SkillId};

fn tool(name: &str, cap: &str, mutating: bool) -> ToolManifest {
    ToolManifest {
        name: name.into(),
        id: format!("tools-fs.{name}"),
        title: name.into(),
        description: format!("Narzędzie testowe {name} do testów kontraktowych umiejętności."),
        input_schema: json!({"type": "object", "additionalProperties": false}),
        output_schema: json!({"type": "object"}),
        reversible: Reversibility::Yes,
        capabilities: vec![cap.into()],
        groups: vec!["fs".into(), cap.into()],
        mutating,
        untrusted_output: None,
    }
}

/// Katalog: `fs_list`, `fs_read` (odczyt), `fs_move`, `fs_write` (zapis).
pub fn sample_catalog() -> Vec<ToolManifest> {
    vec![
        tool("fs_list", "fs.read", false),
        tool("fs_read", "fs.read", false),
        tool("fs_move", "fs.write", true),
        tool("fs_write", "fs.write", true),
    ]
}

/// Umiejętność „porządki w Pobranych” (zapis) w wersji `version`.
pub fn sample_skill(version: &str) -> Skill {
    Skill {
        id: SkillId::new("porzadki-pobranych"),
        version: Version::parse(version).unwrap_or(Version::new(1, 0, 0)),
        name: "Porządki w Pobranych".into(),
        description: "Sortuje folder Pobrane według typu i daty, bez usuwania plików.".into(),
        keywords: vec!["pobrane".into(), "sortowanie".into(), "folder".into()],
        required_tools: vec!["fs_list".into(), "fs_move".into()],
        required_capabilities: vec!["fs.read".into(), "fs.write".into()],
        parameters: json!({
            "type": "object",
            "properties": {
                "folder": {"type": "string", "maxLength": 200},
                "tryb": {"type": "string", "enum": ["typ", "data"], "default": "typ"}
            },
            "required": ["folder"],
            "additionalProperties": false
        }),
        prompt: "Uporządkuj folder {{folder}} według: {{tryb}}.".into(),
        steps: vec![
            "Wylistuj pliki.".into(),
            "Utwórz podfoldery.".into(),
            "Przenieś pliki.".into(),
        ],
        examples: vec![],
        acceptance: vec![
            AcceptanceTest {
                name: "folder w celu".into(),
                params: json!({"folder": "C:/Users/ala/Downloads"}),
                expect_in_goal: vec!["C:/Users/ala/Downloads".into(), "typ".into()],
                expect_rejected: false,
            },
            AcceptanceTest {
                name: "brak folderu".into(),
                params: json!({}),
                expect_in_goal: vec![],
                expect_rejected: true,
            },
        ],
        budget: None,
    }
}
