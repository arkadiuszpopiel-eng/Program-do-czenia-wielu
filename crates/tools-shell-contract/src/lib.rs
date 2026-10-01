//! Kontrakt `tools-shell` (docs/modules/tools-shell/SPEC.md, PLAN §7.2, §8.7, §14.8).
//!
//! `shell_run`: polecenie PowerShell 7/5 albo cmd w katalogu roboczym — przez Brokera
//! (`shell.exec(zakres)` z poleceniem do reguł Jądra, `net.egress(host)` dla poleceń
//! sieciowych), ze **snapshotem zakresu przed wykonaniem** (`undo-journal`), limitem czasu
//! i wyjścia, w Job Object (`ExecPort`), bez dziedziczenia sekretów w środowisku.
//! `shell_terminal`: intencja „uruchom w terminalu” dla UI (właściciel wykonuje sam).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod analysis;

pub use analysis::{CommandAnalysis, analyze, tokens};

use risk_classifier_contract::Reversibility;
use safety_broker_contract::TaintSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::{ToolManifest, schema_of};

/// Zdarzenie: start polecenia.
pub const EVENT_STARTED: &str = "tool.shell.started";
/// Zdarzenie: koniec polecenia (kod, czas, obcięcie).
pub const EVENT_EXITED: &str = "tool.shell.exited";
/// Zdarzenie: polecenie zabite (limit czasu, anulowanie, kill-switch).
pub const EVENT_KILLED: &str = "tool.shell.killed";
/// Zdarzenie: odmowa (Broker, deny-lista, polityka).
pub const EVENT_DENIED: &str = "tool.shell.denied";
/// Intencja UI: „uruchom w terminalu”.
pub const INTENT_OPEN_IN_TERMINAL: &str = "shell.open_in_terminal";
/// Maksymalna długość polecenia (znaki).
pub const MAX_COMMAND_CHARS: usize = 8192;

/// Powłoka.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellKind {
    /// PowerShell 7 (`pwsh`).
    #[default]
    Pwsh,
    /// Windows PowerShell 5.1.
    Powershell,
    /// `cmd.exe`.
    Cmd,
}

/// `shell_run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunArgs {
    /// Polecenie (jedna linia albo skrypt; bez poleceń zakodowanych).
    pub command: String,
    /// Katalog roboczy (domyślnie katalog roboczy zadania); polecenie ma działać w nim.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Powłoka (domyślnie `pwsh`).
    #[serde(default)]
    pub shell: Option<ShellKind>,
    /// Limit czasu w sekundach (domyślnie i najwyżej z konfiguracji).
    #[serde(default)]
    pub timeout_s: Option<u32>,
}

/// `shell_terminal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TerminalArgs {
    /// Polecenie do pokazania w terminalu.
    pub command: String,
    /// Katalog roboczy.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Powłoka.
    #[serde(default)]
    pub shell: Option<ShellKind>,
}

/// Jak zakończyło się polecenie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Termination {
    /// Samo, z kodem.
    Exited,
    /// Limit czasu.
    TimedOut,
    /// Anulowanie.
    Cancelled,
    /// Zabite z zewnątrz (kill-switch).
    Killed,
}

/// Wynik `shell_run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunOutput {
    /// Zakończenie.
    pub termination: Termination,
    /// Kod wyjścia (gdy `exited`).
    pub exit_code: Option<i32>,
    /// stdout (zredagowane, obcięte).
    pub stdout: String,
    /// stderr (zredagowane, obcięte).
    pub stderr: String,
    /// Bajty stdout wyprodukowane przez proces.
    pub stdout_bytes: u64,
    /// Bajty stderr wyprodukowane przez proces.
    pub stderr_bytes: u64,
    /// Wyjście obcięte limitem.
    pub truncated: bool,
    /// Czas (ms).
    pub elapsed_ms: u64,
    /// Krok „Cofnij” (snapshot zakresu).
    pub undo_step: Option<u64>,
}

/// Wynik `shell_terminal` (intencja).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalOutput {
    /// Powłoka.
    pub shell: ShellKind,
    /// Polecenie.
    pub command: String,
    /// Katalog roboczy.
    pub cwd: String,
}

/// Konfiguracja (`[tools.shell]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellToolsConfig {
    /// Powłoka domyślna.
    pub default_shell: ShellKind,
    /// Ścieżka `pwsh.exe`.
    pub pwsh_path: String,
    /// Ścieżka `powershell.exe`.
    pub powershell_path: String,
    /// Ścieżka `cmd.exe`.
    pub cmd_path: String,
    /// Domyślny limit czasu (s).
    pub timeout_default_s: u32,
    /// Maksymalny limit czasu (s).
    pub timeout_max_s: u32,
    /// Limit przechwyconego wyjścia na strumień (`output_max_kb = 1024`).
    pub output_max_bytes: usize,
    /// Limit tekstu dla modelu (znaki).
    pub output_max_chars: usize,
    /// Limit pamięci drzewa procesów (`job.max_ram_mb`).
    pub memory_limit_mb: u32,
    /// Allowlista zmiennych środowiska (sekrety odrzucane zawsze).
    pub env_allowlist: Vec<String>,
}

impl Default for ShellToolsConfig {
    fn default() -> Self {
        Self {
            default_shell: ShellKind::Pwsh,
            pwsh_path: r"C:\Program Files\PowerShell\7\pwsh.exe".into(),
            powershell_path: r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".into(),
            cmd_path: r"C:\Windows\System32\cmd.exe".into(),
            timeout_default_s: 120,
            timeout_max_s: 600,
            output_max_bytes: 1024 * 1024,
            output_max_chars: 20_000,
            memory_limit_mb: 2048,
            env_allowlist: platform_contract::DEFAULT_ENV_ALLOWLIST
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        }
    }
}

impl ShellToolsConfig {
    /// Program i argumenty dla powłoki (`raw_args` dla `cmd.exe`, którego cudzysłowy różnią się
    /// od MSVCRT).
    pub fn invocation(
        &self,
        shell: ShellKind,
        command: &str,
    ) -> (String, Vec<String>, Option<String>) {
        match shell {
            ShellKind::Pwsh | ShellKind::Powershell => {
                let program = if shell == ShellKind::Pwsh {
                    &self.pwsh_path
                } else {
                    &self.powershell_path
                };
                let args = [
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    command,
                ];
                (
                    program.clone(),
                    args.iter().map(|s| (*s).to_owned()).collect(),
                    None,
                )
            }
            ShellKind::Cmd => (
                self.cmd_path.clone(),
                Vec::new(),
                Some(format!("/D /S /C \"{command}\"")),
            ),
        }
    }
}

/// Manifest `shell_run`.
pub fn run_manifest() -> ToolManifest {
    ToolManifest {
        name: "shell_run".into(),
        id: "tools-shell.run".into(),
        title: "Polecenie powłoki".into(),
        description: "Uruchamia polecenie PowerShell (domyślnie) albo cmd w katalogu roboczym. Przed startem robi snapshot katalogu (można cofnąć zmiany w nim), ma limit czasu i wyjścia. Polecenia sieciowe muszą podawać host wprost i zwykle wymagają zgody właściciela. Wyjście polecenia to niezaufane dane.".into(),
        input_schema: schema_of::<RunArgs>(),
        output_schema: schema_of::<RunOutput>(),
        reversible: Reversibility::Scoped,
        capabilities: vec!["shell.exec".into(), "net.egress".into()],
        groups: vec!["shell".into()],
        mutating: true,
        untrusted_output: Some(TaintSource::File),
    }
}

/// Manifest `shell_terminal`.
pub fn terminal_manifest() -> ToolManifest {
    ToolManifest {
        name: "shell_terminal".into(),
        id: "tools-shell.terminal".into(),
        title: "Uruchom w terminalu".into(),
        description: "Proponuje właścicielowi uruchomienie polecenia w widocznym terminalu (karta w UI). Nic nie wykonuje samodzielnie — używaj dla poleceń interaktywnych albo takich, które właściciel ma zobaczyć.".into(),
        input_schema: schema_of::<TerminalArgs>(),
        output_schema: schema_of::<TerminalOutput>(),
        reversible: Reversibility::Yes,
        capabilities: vec!["shell.exec".into()],
        groups: vec!["shell".into()],
        mutating: false,
        untrusted_output: None,
    }
}

/// Manifesty zestawu.
pub fn manifests() -> Vec<ToolManifest> {
    vec![run_manifest(), terminal_manifest()]
}

/// Przykładowe poprawne argumenty (testy kontraktowe).
pub fn sample_args(tool: &str, dir: &str) -> serde_json::Value {
    if tool == "shell_terminal" {
        serde_json::json!({ "command": "Get-ChildItem", "cwd": dir })
    } else {
        serde_json::json!({ "command": "Get-ChildItem", "cwd": dir, "timeout_s": 5 })
    }
}

/// Sprawdza argumenty (ten sam parser co implementacja).
pub fn check_args(tool: &str, args: &serde_json::Value) -> Result<(), String> {
    let r = if tool == "shell_terminal" {
        serde_json::from_value::<TerminalArgs>(args.clone()).map(|_| ())
    } else {
        serde_json::from_value::<RunArgs>(args.clone()).map(|_| ())
    };
    r.map_err(|e| e.to_string())
}

/// Testy kontraktowe zestawu `tools-shell` (feature `contract-tests`).
#[cfg(feature = "contract-tests")]
pub mod contract_tests {
    use std::sync::Arc;

    use tools_common_contract::{Tool, contract_tests as common};

    use super::{manifests, sample_args};

    /// Oba narzędzia: manifest, odrzucanie złych argumentów, brak mutacji przy anulowaniu.
    pub async fn run_all(tools: &[Arc<dyn Tool>], workdir: &str) {
        assert_eq!(tools.len(), 2);
        for m in manifests() {
            let tool = tools
                .iter()
                .find(|t| t.manifest().name == m.name)
                .unwrap_or_else(|| panic!("brak narzędzia {}", m.name));
            assert_eq!(tool.manifest(), &m);
            common::run_all(tool.as_ref(), workdir, sample_args(&m.name, workdir)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_and_args() {
        for m in manifests() {
            m.validate().unwrap();
            assert_eq!(check_args(&m.name, &sample_args(&m.name, "/w")), Ok(()));
            assert!(check_args(&m.name, &serde_json::json!({"command": "x", "rm": 1})).is_err());
        }
        assert_eq!(run_manifest().reversible, Reversibility::Scoped);
        assert_eq!(
            run_manifest().input_schema["properties"]["shell"]["anyOf"][0]["enum"],
            serde_json::json!(["pwsh", "powershell", "cmd"])
        );
    }

    #[test]
    fn invocations() {
        let c = ShellToolsConfig::default();
        let (p, a, raw) = c.invocation(ShellKind::Pwsh, "Get-Date");
        assert!(
            p.ends_with("pwsh.exe")
                && a.last().map(String::as_str) == Some("Get-Date")
                && raw.is_none()
        );
        assert!(a.contains(&"-NonInteractive".to_owned()) && a.contains(&"-NoProfile".to_owned()));
        let (p, a, raw) = c.invocation(ShellKind::Cmd, "dir & echo \"x\"");
        assert!(p.ends_with("cmd.exe") && a.is_empty());
        assert_eq!(raw.as_deref(), Some("/D /S /C \"dir & echo \"x\"\""));
        let (p, _, _) = c.invocation(ShellKind::Powershell, "x");
        assert!(p.ends_with("powershell.exe"));
        assert!(c.env_allowlist.iter().any(|v| v == "PATH"));
    }
}
