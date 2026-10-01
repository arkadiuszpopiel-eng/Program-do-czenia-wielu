//! Izolacja terminala logowania (F4, PLAN §5.5; AGENTS.md): **brak API dla agentek** — od
//! `ui-terminal-*` zależy wyłącznie korzeń kompozycji `app-*`, moduł nie zależy od narzędzi,
//! runtime agentek, MCP, pamięci ani logów i nie implementuje `Tool`/`Toolset`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Nazwy zależności z sekcji `[*dependencies*]` pliku Cargo.toml (prosty parser linii).
fn dependencies(manifest: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_deps = line.contains("dependencies");
            continue;
        }
        if in_deps && let Some((name, _)) = line.split_once('=') {
            let name = name.trim().trim_end_matches(".workspace").to_owned();
            if !name.is_empty() && !name.starts_with('#') {
                deps.push(name);
            }
        }
    }
    deps
}

#[test]
fn only_the_composition_root_depends_on_the_terminal() {
    for entry in std::fs::read_dir(crates_dir()).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(manifest) = std::fs::read_to_string(entry.path().join("Cargo.toml")) else {
            continue;
        };
        let uses_terminal = dependencies(&manifest)
            .iter()
            .any(|d| d.starts_with("ui-terminal-"));
        if uses_terminal {
            assert!(
                name.starts_with("app-") || name.starts_with("ui-terminal-"),
                "{name} zależy od ui-terminal (dozwolone tylko app-*)"
            );
        }
    }
}

#[test]
fn terminal_crates_have_no_agent_log_or_memory_paths() {
    const FORBIDDEN: [&str; 9] = [
        "tools-",
        "agent-",
        "mcp-",
        "memory-",
        "core-log-",
        "tracing",
        "sessions-",
        "search-",
        "providers-",
    ];
    for krate in [
        "ui-terminal-contract",
        "ui-terminal-impl",
        "ui-terminal-fake",
    ] {
        let dir = crates_dir().join(krate);
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        for d in dependencies(&manifest) {
            assert!(
                !FORBIDDEN.iter().any(|f| d.starts_with(f)),
                "{krate} zależy od {d}"
            );
        }
        for file in std::fs::read_dir(dir.join("src")).unwrap().flatten() {
            let src = std::fs::read_to_string(file.path()).unwrap();
            for pattern in [
                "impl Tool for",
                "impl Toolset for",
                "tracing::",
                "println!",
                "eprintln!",
            ] {
                assert!(
                    !src.contains(pattern),
                    "{}: {pattern}",
                    file.path().display()
                );
            }
        }
    }
}
