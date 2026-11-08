//! Dane przykładowe Kreatora (katalog narzędzi, opis z rozmowy, scenariusz testu na sucho) —
//! testy kontraktowe, atrapa, dokumentacja formularza.

use risk_classifier_contract::Reversibility;
use serde_json::json;
use tools_common_contract::ToolManifest;

use crate::draft::{DryExpect, DryScenario, DryStep};

fn tool(
    name: &str,
    caps: &str,
    groups: &[&str],
    mutating: bool,
    rev: Reversibility,
) -> ToolManifest {
    ToolManifest {
        name: name.into(),
        id: format!("tools.{name}"),
        title: name.into(),
        description: format!("Narzędzie {name} do testów kontraktowych Kreatora agentów."),
        input_schema: json!({"type": "object", "additionalProperties": false}),
        output_schema: json!({"type": "object"}),
        reversible: rev,
        capabilities: vec![caps.into()],
        groups: groups.iter().map(|g| (*g).to_owned()).collect(),
        mutating,
        untrusted_output: None,
    }
}

/// Katalog: odczyty i zapisy `fs`, usuwanie, powłoka.
pub fn sample_tools() -> Vec<ToolManifest> {
    vec![
        tool(
            "fs_list",
            "fs.read",
            &["fs", "fs.read"],
            false,
            Reversibility::Yes,
        ),
        tool(
            "fs_read",
            "fs.read",
            &["fs", "fs.read"],
            false,
            Reversibility::Yes,
        ),
        tool(
            "fs_move",
            "fs.write",
            &["fs", "fs.write"],
            true,
            Reversibility::Yes,
        ),
        tool(
            "fs_delete",
            "fs.write",
            &["fs", "fs.write"],
            true,
            Reversibility::Scoped,
        ),
        tool(
            "shell_run",
            "shell.exec",
            &["shell"],
            true,
            Reversibility::Scoped,
        ),
    ]
}

/// Opis z rozmowy (ścieżka szczęśliwa).
pub const DESCRIPTION: &str = "Stwórz agentkę Ola, która sortuje folder Pobrane według typu i daty. Ma być spokojna i dokładna.";

/// Scenariusz testu na sucho dla opisu: lista i przeniesienie w Pobranych — sama; poza
/// zakresem i powłoka — odmowa.
pub fn scenario() -> DryScenario {
    let step = |tool: &str, args: serde_json::Value, expect| DryStep {
        tool: tool.into(),
        args,
        expect,
    };
    DryScenario {
        steps: vec![
            step(
                "fs_list",
                json!({"path": "%USERPROFILE%\\Downloads"}),
                DryExpect::Allowed,
            ),
            step(
                "fs_move",
                json!({"from": "%USERPROFILE%\\Downloads\\a.pdf", "to": "%USERPROFILE%\\Downloads\\PDF\\a.pdf"}),
                DryExpect::Allowed,
            ),
            step(
                "fs_move",
                json!({"from": "%USERPROFILE%\\Downloads\\a.pdf", "to": "%USERPROFILE%\\Documents\\a.pdf"}),
                DryExpect::Denied,
            ),
            step("shell_run", json!({"command": "del *"}), DryExpect::Denied),
        ],
    }
}
