//! S-05 (docs/reviews/2026-10-08-tools-shell-clipboard-net-review.md) na prawdziwych procesach:
//! polecenie złożone tak jak w `shell_run` (`ShellToolsConfig::invocation`) i uruchomione przez
//! prawdziwy `ExecPort` oddaje polskie znaki w UTF-8. Bez przedrostka kodowania Windows PowerShell
//! i `cmd.exe` piszą do potoku w stronie kodowej OEM (CP852) i model dostawał znaki zastępcze.
//! Naprawione dla PowerShella; `cmd.exe` — zob. drugi test.

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use platform_contract::{
    DEFAULT_ENV_ALLOWLIST, ExecControl, ExecPort, ExecSpec, ExecTermination, Integrity,
    ProcessSpec, filter_env,
};
use platform_windows_impl::{JobLimits, WinProcesses};
use tools_shell_contract::{ShellKind, ShellToolsConfig};

const TEXT: &str = "zażółć gęślą jaźń";

/// To samo środowisko co w `shell_run`: nic nie jest dziedziczone poza listą dozwolonych zmiennych.
fn env() -> Vec<(String, String)> {
    let all: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .collect();
    filter_env(all, &DEFAULT_ENV_ALLOWLIST)
}

fn run(shell: ShellKind, command: &str) -> String {
    let (program, args, raw_args) = ShellToolsConfig::default().invocation(shell, command);
    let spec = ExecSpec {
        process: ProcessSpec {
            cmd: PathBuf::from(program),
            args,
            cwd: std::env::temp_dir(),
            integrity: Integrity::Medium,
            memory_limit_mb: None,
        },
        raw_args,
        env: env(),
        timeout_ms: 60_000,
        max_output_bytes: 4_096,
    };
    let out = WinProcesses::new(JobLimits::default())
        .run_captured(spec, &ExecControl::new())
        .expect("start powłoki");
    assert!(
        matches!(out.termination, ExecTermination::Exited(0)),
        "{:?}: {}",
        out.termination,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("wyjście powłoki ma być poprawnym UTF-8")
}

#[test]
fn windows_powershell_output_keeps_polish_characters() {
    let out = run(ShellKind::Powershell, &format!("Write-Output '{TEXT}'"));
    assert_eq!(out.trim(), TEXT);
}

/// `cmd.exe` nadal pisze w stronie kodowej OEM — test utrwala stan faktyczny, żeby zmiana
/// (dekodowanie OEM w warstwie platformy) była świadoma. Otwarta część S-05.
#[test]
fn cmd_output_is_still_in_the_oem_code_page() {
    let (program, args, raw_args) =
        ShellToolsConfig::default().invocation(ShellKind::Cmd, &format!("echo {TEXT}"));
    let spec = ExecSpec {
        process: ProcessSpec {
            cmd: PathBuf::from(program),
            args,
            cwd: std::env::temp_dir(),
            integrity: Integrity::Medium,
            memory_limit_mb: None,
        },
        raw_args,
        env: env(),
        timeout_ms: 60_000,
        max_output_bytes: 4_096,
    };
    let out = WinProcesses::new(JobLimits::default())
        .run_captured(spec, &ExecControl::new())
        .expect("start cmd");
    assert!(matches!(out.termination, ExecTermination::Exited(0)));
    assert!(
        String::from_utf8(out.stdout).is_err(),
        "cmd zaczął pisać UTF-8 — można usunąć zastrzeżenie przy S-05"
    );
}
