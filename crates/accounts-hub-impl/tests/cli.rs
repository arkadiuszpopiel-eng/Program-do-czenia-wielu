//! Wykrywanie mostów CLI (PATH + `--version`) i reguła „kod huba nie odwołuje się do katalogów
//! poświadczeń CLI”.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;

use accounts_hub_contract::EnvSource;
use accounts_hub_impl::ProcessEnv;

#[cfg(unix)]
mod unix {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use accounts_hub_contract::{CliProbe, detect_cli_bridges_with};
    use accounts_hub_impl::SystemCliProbe;

    use super::common;

    fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn detects_bridge_in_path_with_version() {
        let dir = common::temp_dir("cli");
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        write_script(&bin, "claude", "echo '2.3.4 (Claude Code)'");
        std::fs::write(bin.join("codex"), "not executable").unwrap();
        let probe = SystemCliProbe::new(
            Some(std::env::join_paths([Path::new("relative/ignored"), &bin]).unwrap()),
            vec![String::new()],
            Duration::from_secs(5),
        );
        let found = detect_cli_bridges_with(&probe);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "claude");
        assert_eq!(found[0].path, bin.join("claude"));
        assert_eq!(found[0].version.as_deref(), Some("2.3.4"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn hanging_or_failing_version_is_bounded() {
        let dir = common::temp_dir("cli-slow");
        let slow = write_script(&dir, "claude", "sleep 30");
        let failing = write_script(&dir, "codex", "echo 1.0.0; exit 3");
        let probe = SystemCliProbe::new(None, vec![String::new()], Duration::from_millis(200));
        let started = std::time::Instant::now();
        assert_eq!(probe.version_output(&slow), None);
        assert!(started.elapsed() < Duration::from_secs(10));
        assert_eq!(probe.version_output(&failing), None);
        assert_eq!(probe.locate("claude"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn version_probe_runs_with_cleared_environment() {
        // `cargo test` ustawia CARGO_PKG_NAME w procesie testu; dziecko nie może jej dostać
        // (środowisko jest czyszczone do listy dozwolonej — tak samo znikają klucze API).
        let dir = common::temp_dir("cli-env");
        let script = write_script(&dir, "claude", "echo \"9.9.9 ${CARGO_PKG_NAME:-clean}\"");
        let probe = SystemCliProbe::new(None, vec![String::new()], Duration::from_secs(5));
        assert!(std::env::var("CARGO_PKG_NAME").is_ok());
        assert_eq!(
            probe.version_output(&script).as_deref(),
            Some("9.9.9 clean\n")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn process_env_reads_only_requested_variable() {
    assert!(ProcessEnv.var("ALFA_SURELY_UNSET_VARIABLE_1234").is_none());
    assert!(ProcessEnv.var("PATH").is_some());
}

/// Kod crate'ów accounts-hub nie zawiera odwołań do katalogów poświadczeń CLI
/// (AGENTS.md: Alfa nigdy nie czyta ani nie przechowuje tokenów `claude`/`codex`).
#[test]
fn no_cli_credential_paths_in_sources() {
    let forbidden: Vec<String> = ["claude", "codex", "gemini", "grok", "kimi"]
        .iter()
        .flat_map(|cli| [format!("~/.{cli}"), format!(".{cli}/"), format!(".{cli}\\")])
        .chain([
            "\".claude\"".to_owned(),
            "\".codex\"".to_owned(),
            "home_dir".to_owned(),
        ])
        .collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let this_file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("cli.rs")
        .canonicalize()
        .unwrap();
    let mut scanned = 0;
    for krate in [
        "accounts-hub-contract",
        "accounts-hub-impl",
        "accounts-hub-fake",
    ] {
        for sub in ["src", "tests"] {
            let mut stack = vec![root.join(krate).join(sub)];
            while let Some(dir) = stack.pop() {
                let Ok(read) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in read {
                    let path = entry.unwrap().path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }
                    let is_self = path.canonicalize().is_ok_and(|p| p == this_file);
                    if path.extension().is_none_or(|e| e != "rs") || is_self {
                        continue;
                    }
                    let text = std::fs::read_to_string(&path).unwrap();
                    scanned += 1;
                    for needle in &forbidden {
                        assert!(
                            !text.contains(needle.as_str()),
                            "{} zawiera `{needle}`",
                            path.display()
                        );
                    }
                }
            }
        }
    }
    assert!(scanned >= 10, "przeskanowano tylko {scanned} plików");
}
