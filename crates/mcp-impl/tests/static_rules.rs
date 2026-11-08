//! Reguły statyczne (F4-07, AGENTS.md): w źródłach crate'ów `mcp-*` i `agent-backends-*` nie ma
//! nasłuchu sieciowego ani odwołań do katalogów poświadczeń CLI; manifest modułu jest poprawny.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

const CRATES: [&str; 6] = [
    "mcp-contract",
    "mcp-impl",
    "mcp-fake",
    "agent-backends-contract",
    "agent-backends-impl",
    "agent-backends-fake",
];

fn sources() -> Vec<(PathBuf, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let this = Path::new(file!()).file_name().unwrap().to_owned();
    let mut out = Vec::new();
    for krate in CRATES {
        let mut stack = vec![root.join(krate).join("src"), root.join(krate).join("tests")];
        while let Some(dir) = stack.pop() {
            let Ok(read) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in read {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && !(krate == "mcp-impl" && path.file_name() == Some(this.as_os_str()))
                {
                    let text = std::fs::read_to_string(&path).unwrap();
                    out.push((path, text));
                }
            }
        }
    }
    out
}

#[test]
fn no_network_listeners_in_sources() {
    let banned = [
        ["Tcp", "Listener"].concat(),
        ["Tcp", "Stream"].concat(),
        ["Udp", "Socket"].concat(),
        ["std::", "net::"].concat(),
        ["tokio::", "net::Tcp"].concat(),
        ["0.0.0.0", ""].concat(),
        ["127.0.0.1:", ""].concat(),
    ];
    let files = sources();
    assert!(
        files.len() >= 20,
        "przeskanowano tylko {} plików",
        files.len()
    );
    for (path, text) in &files {
        for needle in &banned {
            assert!(
                !text.contains(needle.as_str()),
                "{} zawiera `{needle}`",
                path.display()
            );
        }
    }
}

#[test]
fn no_cli_credential_paths_in_sources() {
    let mut banned: Vec<String> = ["claude", "codex", "gemini", "grok", "kimi"]
        .iter()
        .flat_map(|cli| {
            [
                format!("~/.{cli}"),
                format!(".{cli}/"),
                format!(".{cli}\\"),
                format!("\".{cli}\""),
            ]
        })
        .collect();
    banned.extend([
        ["home", "_dir"].concat(),
        ["Cookies", ""].concat(),
        ["CredRead", ""].concat(),
    ]);
    for (path, text) in &sources() {
        // Skaner injection (mcp-contract/src/injection.rs) wykrywa te frazy w opisach narzędzi.
        if path.ends_with("injection.rs") {
            continue;
        }
        for needle in &banned {
            assert!(
                !text.contains(needle.as_str()),
                "{} zawiera `{needle}`",
                path.display()
            );
        }
    }
}

#[test]
fn module_manifest_is_valid() {
    let m = core_registry_contract::ModuleManifest::parse_toml(mcp_impl::MODULE_TOML).unwrap();
    assert_eq!(m.id.to_string(), "mcp");
}
