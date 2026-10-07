//! Reguły modułu: manifest, walidacja zadań, skan źródeł mostów (brak odczytu poświadczeń CLI
//! i kluczy z środowiska, brak trybów omijających zatwierdzenia).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use agent_backends_contract::{BridgeKind, SessionRef, TaskSpec};
use agent_backends_impl::gate::validate;
use core_bus_contract::SessionId;

#[test]
fn module_manifest_is_valid() {
    let m = core_registry_contract::ModuleManifest::parse_toml(agent_backends_impl::MODULE_TOML)
        .unwrap();
    assert_eq!(m.id.to_string(), "agent-backends");
}

/// Most Claude Code dostaje serwer MCP Alfy v1 (UIA, zrzut, rejestr przez Brokera) w imieniu
/// sesji rozmowy, z której zlecono zadanie — nie osobnej sesji `most:<zadanie>`.
#[test]
fn mcp_scope_is_v1_bound_to_the_chat_session() {
    let spec = TaskSpec::user_request(
        BridgeKind::ClaudeCode,
        "zrób",
        "/x",
        SessionId::new("czat-7"),
    );
    let scope =
        agent_backends_impl::mcp_scope(&agent_backends_contract::TaskId("task-1".into()), &spec);
    assert_eq!(scope.label, "task-1");
    assert_eq!(scope.session.as_deref(), Some("czat-7"));
    for t in mcp_contract::AlfaTool::WINDOWS_V1 {
        assert!(scope.tools.contains(&t), "{t:?}");
    }
    assert!(scope.tools.contains(&mcp_contract::AlfaTool::RegistryRead));
}

#[test]
fn spec_validation() {
    let ok = TaskSpec::user_request(BridgeKind::ClaudeCode, "zrób", "/x", SessionId::new("s"));
    assert!(validate(&ok).is_ok());
    let mut long = ok.clone();
    long.prompt = "x".repeat(agent_backends_impl::gate::MAX_PROMPT_CHARS + 1);
    assert!(validate(&long).is_err());
    let mut ctl = ok.clone();
    ctl.disallowed_tools = vec!["Bash\n--x".into()];
    assert!(validate(&ctl).is_err());
    let mut session = ok.clone();
    session.session = Some(SessionRef {
        bridge: BridgeKind::ClaudeCode,
        id: "--dangerously-skip-permissions".into(),
        workdir: "/x".into(),
    });
    assert!(validate(&session).is_err());
    let mut other = ok;
    other.session = Some(SessionRef {
        bridge: BridgeKind::Codex,
        id: "s".into(),
        workdir: "/x".into(),
    });
    assert!(validate(&other).is_err());
}

/// Skan źródeł crate'ów `agent-backends-*`: żadnych ścieżek poświadczeń CLI, odczytu kluczy
/// dostawców ze środowiska ani flag omijających zatwierdzenia (AGENTS.md, PLAN §1.3).
#[test]
fn sources_never_touch_cli_credentials() {
    let banned: Vec<String> = ["claude", "codex", "gemini", "grok", "kimi"]
        .iter()
        .flat_map(|c| {
            [
                format!("~/.{c}"),
                format!(".{c}/"),
                format!(".{c}\\"),
                format!("\".{c}\""),
            ]
        })
        .chain([
            ["home", "_dir"].concat(),
            ["var(\"ANTHROPIC", ""].concat(),
            ["var(\"OPENAI", ""].concat(),
            ["dangerously", "-skip-permissions"].concat(),
            ["bypass", "Permissions"].concat(),
            ["\"never\"", ""].concat(),
            ["danger-full", "-access"].concat(),
            ["credentials", ".json"].concat(),
            ["auth", ".json"].concat(),
        ])
        .collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let me = Path::new(file!()).file_name().unwrap().to_owned();
    let mut scanned = 0;
    for krate in [
        "agent-backends-contract",
        "agent-backends-impl",
        "agent-backends-fake",
    ] {
        let mut stack = vec![root.join(krate).join("src"), root.join(krate).join("tests")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs")
                    || path.file_name() == Some(me.as_os_str())
                {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                scanned += 1;
                for needle in &banned {
                    assert!(
                        !text.contains(needle.as_str()),
                        "{} zawiera `{needle}`",
                        path.display()
                    );
                }
            }
        }
    }
    assert!(scanned >= 20, "przeskanowano tylko {scanned} plików");
}
