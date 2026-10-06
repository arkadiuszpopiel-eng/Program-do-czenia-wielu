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
                    log_server_line(&line);
                }
            });
        }
        Ok(Box::new(TokioProcess(child)))
    }
}

/// Poziom wiersza stderr `llama-server` w dzienniku Alfy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineLevel {
    /// Błąd serwera (np. brak pamięci karty) — `warn`, widoczny domyślnie.
    Warn,
    /// Architektura modelu, KV cache, odciążenie warstw, urządzenie — `info`, widoczny domyślnie
    /// (potwierdzenie szacunków VRAM z `models.toml`).
    Info,
    /// Reszta — `debug` pod osobnym celem `llama_server` (tylko po jawnym włączeniu:
    /// `ALFA_LOG=info,llama_server=debug`; wiersze żądań mogą nieść treść).
    Debug,
}

const ERROR_MARKERS: [&str; 6] = [
    "error",
    "failed",
    "out of memory",
    "exception",
    "abort",
    "fatal",
];

const INFO_MARKERS: [&str; 13] = [
    "n_layer",
    "n_head_kv",
    "n_embd ",
    "n_ctx_train",
    "n_vocab",
    "file type",
    "model params",
    "kv_cache",
    "offloaded",
    "ggml_cuda_init",
    "using device",
    "model buffer size",
    "server is listening",
];

pub(crate) fn classify_line(line: &str) -> LineLevel {
    let lower = line.to_ascii_lowercase();
    if ERROR_MARKERS.iter().any(|m| lower.contains(m)) {
        LineLevel::Warn
    } else if INFO_MARKERS.iter().any(|m| line.contains(m)) {
        LineLevel::Info
    } else {
        LineLevel::Debug
    }
}

fn log_server_line(line: &str) {
    match classify_line(line) {
        LineLevel::Warn => tracing::warn!(target: "providers_local_impl::llama_server", "{line}"),
        LineLevel::Info => tracing::info!(target: "providers_local_impl::llama_server", "{line}"),
        LineLevel::Debug => tracing::debug!(target: "llama_server", "{line}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{LineLevel, classify_line};

    #[test]
    fn server_lines_are_classified_for_the_log() {
        for (line, level) in [
            (
                "ggml_backend_cuda_buffer_type_alloc_buffer: allocating 5120.00 MiB on device 0: cudaMalloc failed: out of memory",
                LineLevel::Warn,
            ),
            (
                "llama_model_load: error loading model: tensor data is not within file bounds",
                LineLevel::Warn,
            ),
            ("print_info: n_layer          = 60", LineLevel::Info),
            (
                "llama_kv_cache:      CUDA0 KV buffer size =   272.00 MiB",
                LineLevel::Info,
            ),
            (
                "load_tensors: offloaded 34/61 layers to GPU",
                LineLevel::Info,
            ),
            (
                "main: server is listening on http://127.0.0.1:52011",
                LineLevel::Info,
            ),
            (
                "srv  log_server_r: request: POST /v1/chat/completions 127.0.0.1 200",
                LineLevel::Debug,
            ),
            (
                "slot launch_slot_: id  0 | task 12 | processing task",
                LineLevel::Debug,
            ),
        ] {
            assert_eq!(classify_line(line), level, "{line}");
        }
    }
}
