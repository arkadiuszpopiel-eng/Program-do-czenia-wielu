//! Porty systemowe: wykrywanie mostów CLI w PATH (`--version`), zmienne środowiskowe procesu.
//!
//! Wykrywanie mostów sprawdza wyłącznie istnienie pliku wykonywalnego w PATH i jego wersję.
//! Ten moduł nie otwiera żadnych katalogów konfiguracji ani plików z tokenami narzędzi CLI
//! (test `no_cli_credential_paths` sprawdza to w kodzie źródłowym crate'ów accounts-hub).

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use accounts_hub_contract::{
    CliBridge, CliProbe, EnvSource, SecretString, detect_cli_bridges_with,
};

/// Zmienne przekazywane do `--version` (reszta środowiska, w tym klucze API, jest czyszczona).
const CHILD_ENV_ALLOWLIST: [&str; 12] = [
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "TEMP",
    "TMP",
    "HOME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
];

/// Maksymalna liczba bajtów czytanych z wyjścia `--version`.
const MAX_VERSION_OUTPUT: u64 = 4096;

/// Wykrywanie CLI przez PATH procesu.
#[derive(Debug, Clone)]
pub struct SystemCliProbe {
    path: Option<OsString>,
    extensions: Vec<String>,
    timeout: Duration,
}

impl SystemCliProbe {
    /// PATH i PATHEXT z bieżącego procesu, limit `--version` 5 s.
    pub fn from_env() -> Self {
        let extensions = if cfg!(windows) {
            std::env::var("PATHEXT")
                .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
                .split(';')
                .filter(|e| !e.is_empty())
                .map(str::to_owned)
                .collect()
        } else {
            vec![String::new()]
        };
        Self::new(std::env::var_os("PATH"), extensions, Duration::from_secs(5))
    }

    /// Jawne PATH, rozszerzenia (np. `["", ".exe"]`) i limit czasu.
    pub fn new(path: Option<OsString>, extensions: Vec<String>, timeout: Duration) -> Self {
        Self {
            path,
            extensions,
            timeout,
        }
    }
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

impl CliProbe for SystemCliProbe {
    fn locate(&self, program: &str) -> Option<PathBuf> {
        let path = self.path.as_ref()?;
        std::env::split_paths(path)
            .filter(|dir| dir.is_absolute())
            .flat_map(|dir| {
                self.extensions
                    .iter()
                    .map(move |ext| dir.join(format!("{program}{ext}")))
            })
            .find(|candidate| is_executable(candidate))
    }

    fn version_output(&self, path: &Path) -> Option<String> {
        let mut cmd = Command::new(path);
        cmd.arg("--version")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for name in CHILD_ENV_ALLOWLIST {
            if let Some(value) = std::env::var_os(name) {
                cmd.env(name, value);
            }
        }
        let mut child = cmd.spawn().ok()?;
        let stdout = child.stdout.take()?;
        let reader = std::thread::spawn(move || {
            let mut buf = Vec::new();
            stdout.take(MAX_VERSION_OUTPUT).read_to_end(&mut buf).ok()?;
            Some(buf)
        });
        let deadline = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };
        // Po przekroczeniu czasu nie czekamy na wątek czytający: wnuk procesu (np. skrypt
        // powłoki) może trzymać otwarty potok; wątek zakończy się sam po jego zamknięciu.
        status.filter(|s| s.success())?;
        let output = reader.join().ok().flatten()?;
        Some(String::from_utf8_lossy(&output).into_owned())
    }
}

/// Wykrywa zainstalowane mosty CLI (`claude`, `codex`) w PATH bieżącego procesu.
pub fn detect_cli_bridges() -> Vec<CliBridge> {
    detect_cli_bridges_with(&SystemCliProbe::from_env())
}

/// Zmienne środowiskowe bieżącego procesu (import kluczy na życzenie użytkownika).
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn var(&self, name: &str) -> Option<SecretString> {
        std::env::var(name)
            .ok()
            .map(SecretString::new)
            .filter(|s| !s.is_empty())
    }
}
