//! S-05 (docs/reviews/2026-10-08-tools-shell-clipboard-net-review.md) na prawdziwych procesach:
//! polecenie złożone tak jak w `shell_run` (`ShellToolsConfig::invocation`) i uruchomione przez
//! prawdziwy `ExecPort` oddaje polskie znaki w UTF-8. Bez przedrostka kodowania Windows PowerShell
//! i `cmd.exe` piszą do potoku w stronie kodowej OEM systemu i model dostawał znaki zastępcze.
//! PowerShell dostaje przedrostek kodowania; wyjście `cmd.exe` dekoduje warstwa platformy.

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

/// Konfiguracja OEM i koder .NET tworzą dane emitera niezależnie od dekodera `ExecPort`.
/// Port PowerShell jest już używany przez ten test; kod Rust zachowuje zakaz `unsafe`.
fn oem_fixture() -> (u32, String, Vec<u8>) {
    let report = run(
        ShellKind::Powershell,
        r#"
$cp = [int][Microsoft.Win32.Registry]::GetValue('HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Nls\CodePage', 'OEMCP', $null)
if ($cp -le 0) { throw 'Brak konfiguracji OEM' }
$enc = [Text.Encoding]::GetEncoding($cp, [Text.EncoderExceptionFallback]::new(), [Text.DecoderExceptionFallback]::new())
$strictUtf8 = [Text.UTF8Encoding]::new($false, $true)
foreach ($text in @('zażółć gęślą jaźń', 'é', 'ü', 'Ж', 'Ω', 'א', 'ا', 'ก', 'あ', '中', '한')) {
    try { $encoded = $enc.GetBytes($text) } catch [Text.EncoderFallbackException] { continue }
    $isUtf8 = $true
    try { $null = $strictUtf8.GetString($encoded) } catch [Text.DecoderFallbackException] { $isUtf8 = $false }
    if ($cp -ne 65001 -and $isUtf8) { continue }
    [Console]::WriteLine($cp)
    [Console]::WriteLine([BitConverter]::ToString([Text.Encoding]::UTF8.GetBytes($text)).Replace('-', ''))
    [Console]::WriteLine([BitConverter]::ToString($encoded).Replace('-', ''))
    return
}
throw 'Brak reprezentowalnego znaku spoza ASCII dla OEM'
"#,
    );
    let mut lines = report.lines().filter(|line| !line.trim().is_empty());
    let cp = lines
        .next()
        .expect("strona OEM")
        .trim()
        .parse()
        .expect("numer OEM");
    let text = String::from_utf8(unhex(lines.next().expect("tekst UTF-8"))).expect("tekst fixture");
    let encoded = unhex(lines.next().expect("bajty OEM"));
    assert!(
        lines.next().is_none(),
        "nieoczekiwane dodatkowe dane fixture"
    );
    assert!(!encoded.is_empty());
    (cp, text, encoded)
}

fn unhex(text: &str) -> Vec<u8> {
    let text = text.trim();
    assert!(text.len().is_multiple_of(2));
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

/// `cmd.exe` emituje znaki reprezentowalne w OEM, a `ExecPort` odzyskuje oryginalny tekst.
#[test]
fn cmd_output_is_decoded_from_the_oem_code_page() {
    let (cp, expected, encoded) = oem_fixture();
    let (program, args, raw_args) =
        ShellToolsConfig::default().invocation(ShellKind::Cmd, &format!("echo {expected}"));
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
    let exec = WinProcesses::new(JobLimits::default());
    let out = exec
        .run_captured(spec, &ExecControl::new())
        .expect("start cmd");
    assert!(matches!(out.termination, ExecTermination::Exited(0)));
    assert_eq!(
        out.stdout.trim_ascii(),
        encoded.as_slice(),
        "emiter OEM{cp}"
    );
    if cp != 65001 {
        assert!(
            String::from_utf8(out.stdout.clone()).is_err(),
            "wejście OEM{cp} ma wymagać dekodowania OEM"
        );
    }
    assert_eq!(exec.decode_output(&out.stdout).trim(), expected);
}
