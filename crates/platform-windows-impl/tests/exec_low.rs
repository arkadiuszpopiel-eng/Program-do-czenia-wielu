//! Polecenie o niskiej integralności na prawdziwych procesach (dokument 25 §1, audyt A-07):
//! zapis w katalogu roboczym działa, zapis obok (zwykły katalog użytkownika) system odrzuca,
//! odczyt plików systemu i przechwycenie wyjścia działają jak przy integralności Medium.

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use platform_contract::{
    DEFAULT_ENV_ALLOWLIST, ExecControl, ExecOutput, ExecPort, ExecSpec, ExecTermination, Integrity,
    ProcessSpec, filter_env,
};
use platform_windows_impl::{JobLimits, WinProcesses};

fn env() -> Vec<(String, String)> {
    let all: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .collect();
    filter_env(all, &DEFAULT_ENV_ALLOWLIST)
}

fn cmd(cwd: &Path, line: &str, integrity: Integrity) -> ExecOutput {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let spec = ExecSpec {
        process: ProcessSpec {
            cmd: PathBuf::from(root).join(r"System32\cmd.exe"),
            args: Vec::new(),
            cwd: cwd.to_path_buf(),
            integrity,
            memory_limit_mb: None,
        },
        raw_args: Some(format!("/d /s /c \"{line}\"")),
        env: env(),
        timeout_ms: 60_000,
        max_output_bytes: 8_192,
    };
    WinProcesses::new(JobLimits::default())
        .run_captured(spec, &ExecControl::new())
        .expect("start polecenia")
}

fn dirs(tag: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("alfa-low-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let work = base.join("praca");
    let beside = base.join("obok");
    std::fs::create_dir_all(work.join("pod")).unwrap();
    std::fs::create_dir_all(&beside).unwrap();
    std::fs::write(work.join("istniejacy.txt"), "stary").unwrap();
    (work, beside)
}

#[test]
fn low_integrity_writes_only_inside_the_workdir() {
    let (work, beside) = dirs("zapis");
    let outside = beside.join("poza.txt");
    let line = format!(
        "echo ok> wewnatrz.txt & echo nowy> istniejacy.txt & echo glebiej> pod\\plik.txt & \
         echo x> \"{}\" & type \"%SystemRoot%\\win.ini\" >nul && echo odczyt-ok",
        outside.display()
    );
    let out = cmd(&work, &line, Integrity::Low);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        matches!(out.termination, ExecTermination::Exited(_)),
        "{out:?}"
    );
    // W katalogu roboczym: nowy plik, nadpisany istniejący (etykieta przeszła na drzewo), podkatalog.
    assert_eq!(
        std::fs::read_to_string(work.join("wewnatrz.txt"))
            .unwrap()
            .trim(),
        "ok"
    );
    assert_eq!(
        std::fs::read_to_string(work.join("istniejacy.txt"))
            .unwrap()
            .trim(),
        "nowy"
    );
    assert!(work.join("pod").join("plik.txt").exists(), "{stderr}");
    // Obok: odmowa dostępu systemu (No-Write-Up), pliku brak.
    assert!(
        !outside.exists(),
        "proces Low zapisał poza katalogiem roboczym"
    );
    assert!(!stderr.is_empty(), "odmowa zapisu powinna trafić do stderr");
    // Odczyt plików systemu działa.
    assert!(stdout.contains("odczyt-ok"), "{stdout} / {stderr}");
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}

#[test]
fn medium_integrity_is_unchanged() {
    let (work, beside) = dirs("medium");
    let outside = beside.join("poza.txt");
    let out = cmd(
        &work,
        &format!("echo x> \"{}\"", outside.display()),
        Integrity::Medium,
    );
    assert!(
        matches!(out.termination, ExecTermination::Exited(0)),
        "{out:?}"
    );
    assert!(outside.exists());
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}

#[test]
fn low_integrity_temp_is_writable() {
    let (work, _) = dirs("temp");
    let out = cmd(
        &work,
        "echo t> \"%TEMP%\\alfa-low-probe.txt\" && echo temp-ok && del \"%TEMP%\\alfa-low-probe.txt\"",
        Integrity::Low,
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("temp-ok"),
        "{stdout} / {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}

/// Domyślna powłoka agentek (Windows PowerShell, jak w `shell_run`) działa o niskiej integralności:
/// zapis i odczyt w katalogu roboczym, polskie znaki w wyjściu.
#[test]
fn powershell_runs_at_low_integrity() {
    use tools_shell_contract::{ShellKind, ShellToolsConfig};
    let (work, beside) = dirs("pwsh");
    let outside = beside.join("poza.txt");
    let command = format!(
        "Set-Content -Path wynik.txt -Value 'zażółć'; Get-Content wynik.txt; \
         try {{ Set-Content -Path '{}' -Value x -ErrorAction Stop; 'zapisano-poza' }} catch {{ 'odmowa-poza' }}",
        outside.display()
    );
    let (program, args, raw_args) =
        ShellToolsConfig::default().invocation(ShellKind::Powershell, &command);
    let spec = ExecSpec {
        process: ProcessSpec {
            cmd: PathBuf::from(program),
            args,
            cwd: work.clone(),
            integrity: Integrity::Low,
            memory_limit_mb: None,
        },
        raw_args,
        env: env(),
        timeout_ms: 120_000,
        max_output_bytes: 8_192,
    };
    let out = WinProcesses::new(JobLimits::default())
        .run_captured(spec, &ExecControl::new())
        .expect("start PowerShell");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("zażółć"),
        "{stdout} / {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("odmowa-poza"), "{stdout}");
    assert!(!outside.exists());
    let _ = std::fs::remove_dir_all(work.parent().unwrap());
}
