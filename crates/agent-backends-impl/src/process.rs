//! Procesy CLI: uruchamianie ze zredukowanym środowiskiem i potokami stdio, zabijanie drzewa
//! procesów, `--version`, hash pliku wykonywalnego, wyszukiwanie w PATH.
//!
//! `platform-contract::ProcessPort` nie ma (jeszcze) uruchamiania z potokami stdio, więc procesy
//! startuje `tokio::process`, a drzewo zabija [`TreeKiller`]: domyślnie grupa procesów (Unix)
//! albo `taskkill /T /F` (Windows). Kompozycja może podać implementację na Job Object
//! z `platform-windows-impl`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use agent_backends_contract::{BackendError, child_env};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

/// Maksymalna liczba bajtów czytanych z `--version`.
const MAX_VERSION_OUTPUT: u64 = 4096;

/// Zabija całe drzewo procesów o danym PID.
pub trait TreeKiller: Send + Sync {
    /// Zabija drzewo (najlepszy wysiłek; błędy są logowane przez wywołującego).
    fn kill_tree(&self, pid: u32) -> Result<(), String>;
}

/// Domyślny zabójca drzew.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemTreeKiller;

impl TreeKiller for SystemTreeKiller {
    #[cfg(unix)]
    fn kill_tree(&self, pid: u32) -> Result<(), String> {
        // Proces CLI startuje we własnej grupie (pgid = pid) — zabijamy całą grupę.
        let status = std::process::Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("kill zwrócił {status}"))
        }
    }

    #[cfg(windows)]
    fn kill_tree(&self, pid: u32) -> Result<(), String> {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        let taskkill = PathBuf::from(root).join("System32").join("taskkill.exe");
        let status = std::process::Command::new(taskkill)
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("taskkill zwrócił {status}"))
        }
    }
}

/// Środowisko procesu CLI: lista dozwolona z bieżącego środowiska, bez sekretów Alfy.
pub fn cli_env() -> std::collections::BTreeMap<String, String> {
    child_env(std::env::vars())
}

/// Polecenie z czystym środowiskiem, katalogiem roboczym i własną grupą procesów.
pub fn command(program: &Path, args: &[String], cwd: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(cli_env())
        .kill_on_drop(true);
    #[cfg(unix)]
    cmd.process_group(0);
    cmd
}

/// Uruchamia CLI z potokami stdin/stdout/stderr.
pub fn spawn_piped(
    program: &Path,
    args: &[String],
    cwd: &Path,
) -> Result<tokio::process::Child, BackendError> {
    command(program, args, cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BackendError::Spawn(e.to_string()))
}

/// `<program> --version` (czyste środowisko, limit czasu, kod 0) → wersja `x.y.z`.
pub async fn probe_version(program: &Path, timeout: Duration) -> Option<String> {
    let cwd = std::env::temp_dir();
    let mut child = command(program, &["--version".to_owned()], &cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let run = async {
        let mut buf = Vec::new();
        let _ = (&mut stdout)
            .take(MAX_VERSION_OUTPUT)
            .read_to_end(&mut buf)
            .await;
        let status = child.wait().await.ok()?;
        status.success().then_some(buf)
    };
    let out = match tokio::time::timeout(timeout, run).await {
        Ok(out) => out?,
        Err(_) => {
            if let Some(pid) = child.id() {
                let _ = SystemTreeKiller.kill_tree(pid);
            }
            let _ = child.start_kill();
            return None;
        }
    };
    accounts_hub_contract::parse_version(&String::from_utf8_lossy(&out))
}

/// SHA-256 pliku wykonywalnego CLI (hex, małe litery).
pub async fn sha256_file(path: &Path) -> Result<String, BackendError> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| BackendError::Spawn(format!("nie można odczytać programu: {e}")))?;
    let digest = Sha256::digest(&bytes);
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// Ścieżka programu: bezwzględna (musi istnieć) albo wyszukana w PATH (z PATHEXT na Windows).
pub fn locate(program: &Path) -> Option<PathBuf> {
    if program.is_absolute() {
        return program.is_file().then(|| program.to_path_buf());
    }
    if program.components().count() != 1 {
        return None;
    }
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(str::to_owned)
            .chain([String::new()])
            .collect()
    } else {
        vec![String::new()]
    };
    let name = program.to_string_lossy();
    std::env::split_paths(&path)
        .filter(|d| d.is_absolute())
        .flat_map(|dir| {
            exts.iter()
                .map(|e| dir.join(format!("{name}{e}")))
                .collect::<Vec<_>>()
        })
        .find(|c| c.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_rules() {
        assert_eq!(locate(Path::new("/nie/ma/takiego")), None);
        assert_eq!(locate(Path::new("a/b")), None);
        assert_eq!(
            locate(Path::new("na-pewno-nie-ma-takiego-programu-xyz")),
            None
        );
    }

    #[test]
    fn env_has_no_secrets() {
        for name in cli_env().keys() {
            assert!(
                !agent_backends_contract::policy::is_secret_env_name(name),
                "{name}"
            );
        }
    }

    #[tokio::test]
    async fn version_of_missing_program_is_none() {
        assert_eq!(
            probe_version(Path::new("/nie/ma/programu"), Duration::from_millis(200)).await,
            None
        );
        assert!(sha256_file(Path::new("/nie/ma/programu")).await.is_err());
    }
}
