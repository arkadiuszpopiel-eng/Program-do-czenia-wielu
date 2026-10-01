//! Uruchamianie procesu sidecara (trait dla testów i przyszłego Job Object z `platform-windows`).

use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::LocalError;

/// Specyfikacja uruchomienia (`Debug` redaguje sekret i argumenty).
#[derive(Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    /// Program.
    pub program: PathBuf,
    /// Argumenty.
    pub args: Vec<String>,
    /// Sekret redagowany w logach procesu (klucz API sidecara).
    pub secret: String,
}

impl std::fmt::Debug for LaunchSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaunchSpec")
            .field("program", &self.program)
            .field("args", &self.args.len())
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

/// Uruchomiony proces.
pub trait SidecarProcess: Send {
    /// PID (diagnostyka).
    fn pid(&self) -> Option<u32>;
    /// `Some(kod)` gdy proces się zakończył (bez czekania).
    fn exited(&mut self) -> Option<Option<i32>>;
    /// Zabija proces (idempotentne).
    fn kill(&mut self);
}

/// Uruchamiacz procesów.
pub trait SidecarLauncher: Send + Sync {
    /// Uruchamia proces.
    fn spawn(&self, spec: &LaunchSpec) -> Result<Box<dyn SidecarProcess>, LocalError>;
}

/// Uruchamiacz `tokio::process` (bez okna konsoli na Windows, `kill_on_drop`, stderr → logi).
#[derive(Debug, Default, Clone)]
pub struct TokioLauncher {
    /// Dodatkowe zmienne środowiskowe (testy: sterowanie fałszywym serwerem).
    pub env: Vec<(String, String)>,
}

struct TokioProcess(tokio::process::Child);

impl SidecarProcess for TokioProcess {
    fn pid(&self) -> Option<u32> {
        self.0.id()
    }

    fn exited(&mut self) -> Option<Option<i32>> {
        match self.0.try_wait() {
            Ok(Some(status)) => Some(status.code()),
            Ok(None) => None,
            Err(_) => Some(None),
        }
    }

    fn kill(&mut self) {
        // Błąd = proces już nie żyje.
        let _ = self.0.start_kill();
    }
}

/// Flaga `CREATE_NO_WINDOW` (bez okna konsoli dla procesu potomnego).
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

impl SidecarLauncher for TokioLauncher {
    fn spawn(&self, spec: &LaunchSpec) -> Result<Box<dyn SidecarProcess>, LocalError> {
        let mut cmd = tokio::process::Command::new(&spec.program);
        cmd.args(&spec.args)
            .envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd
            .spawn()
            .map_err(|e| LocalError::Spawn(format!("{}: {e}", spec.program.display())))?;
        if let Some(stderr) = child.stderr.take() {
            let secret = spec.secret.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let line = if secret.is_empty() {
                        line
                    } else {
                        line.replace(&secret, "[REDACTED]")
                    };
                    tracing::debug!(target: "llama_server", "{line}");
                }
            });
        }
        Ok(Box::new(TokioProcess(child)))
    }
}
