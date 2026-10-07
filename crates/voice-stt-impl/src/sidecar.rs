//! Sidecar `whisper-server`: argumenty z profilu, uruchamianie (proces z przechwyconym stderr),
//! losowy port na 127.0.0.1, wykrywanie awarii GPU (`ErrorDeviceLost`).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;
use device_profile_contract::Backend;
use tokio::io::{AsyncBufReadExt, BufReader};
use voice_stt_contract::SttError;

/// Wpisy stderr świadczące o utracie urządzenia GPU (Vulkan/CUDA) → fallback CPU.
pub const DEVICE_LOST_MARKERS: [&str; 6] = [
    "ErrorDeviceLost",
    "VK_ERROR_DEVICE_LOST",
    "DeviceLost",
    "CUDA error",
    "cudaErrorMemoryAllocation",
    "out of memory",
];

/// Binaria `whisper-server` (whisper.cpp ≥ 1.8.1, przypięte) per backend.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SidecarBinaries {
    /// Build z Vulkanem (AMD/Intel/NVIDIA).
    pub vulkan: Option<PathBuf>,
    /// Build z CUDA (NVIDIA).
    pub cuda: Option<PathBuf>,
    /// Build CPU (zawsze wymagany — zapas).
    pub cpu: PathBuf,
}

impl SidecarBinaries {
    /// Binarium dla backendu (`None` = brak buildu → CPU).
    pub fn for_backend(&self, backend: Backend) -> Option<&PathBuf> {
        match backend {
            Backend::Vulkan => self.vulkan.as_ref(),
            Backend::Cuda => self.cuda.as_ref(),
            Backend::Cpu => Some(&self.cpu),
        }
    }
}

/// Konfiguracja sidecara (`[machine.voice.stt]`).
#[derive(Debug, Clone, PartialEq)]
pub struct WhisperServerConfig {
    /// Binaria.
    pub binaries: SidecarBinaries,
    /// Plik modelu GGML (`ggml-large-v3-turbo-q5_0.bin`, hash sprawdza instalator modeli).
    pub model_path: PathBuf,
    /// Wątki CPU.
    pub threads: u16,
    /// Limit startu (ładowanie modelu).
    pub startup_timeout: Duration,
    /// Limit pojedynczego żądania.
    pub request_timeout: Duration,
    /// Maksymalna liczba restartów na jedno żądanie (awaria → ponowienie / CPU).
    pub max_restarts: u32,
    /// Zwolnienie po bezczynności (ms; `model-residency`).
    pub idle_unload_ms: u64,
}

impl WhisperServerConfig {
    /// Konfiguracja z domyślnymi limitami.
    pub fn new(binaries: SidecarBinaries, model_path: PathBuf) -> Self {
        Self {
            binaries,
            model_path,
            threads: 4,
            startup_timeout: Duration::from_secs(60),
            request_timeout: Duration::from_secs(30),
            max_restarts: 2,
            idle_unload_ms: 600_000,
        }
    }
}

/// Argumenty `whisper-server` dla backendu i portu.
pub fn build_args(cfg: &WhisperServerConfig, backend: Backend, port: u16) -> Vec<String> {
    let mut args = vec![
        "-m".to_owned(),
        cfg.model_path.display().to_string(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        port.to_string(),
        "-t".into(),
        cfg.threads.to_string(),
        // Bez osobnego przebiegu prawdopodobieństw języków (drogi); język wraca w `language`.
        "-nlp".into(),
        // Tłumienie tokenów nie-mowy (mniej halucynacji na szumie).
        "-sns".into(),
    ];
    match backend {
        Backend::Cpu => args.push("-ng".into()),
        Backend::Vulkan | Backend::Cuda => args.push("-fa".into()),
    }
    args
}

/// Wolny port na 127.0.0.1 (bind na :0 i zwolnienie — sidecar zajmuje go chwilę później).
pub fn free_port() -> Result<u16, SttError> {
    let l =
        std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| SttError::Sidecar(e.to_string()))?;
    l.local_addr()
        .map(|a| a.port())
        .map_err(|e| SttError::Sidecar(e.to_string()))
}

/// Co i jak uruchomić.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    /// Program.
    pub program: PathBuf,
    /// Argumenty.
    pub args: Vec<String>,
    /// Port HTTP.
    pub port: u16,
    /// Backend.
    pub backend: Backend,
}

/// Zakończenie procesu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitInfo {
    /// Kod wyjścia.
    pub code: Option<i32>,
    /// Utrata urządzenia GPU wg stderr.
    pub device_lost: bool,
}

/// Uruchomiony sidecar.
pub trait Sidecar: Send + Sync {
    /// Adres bazowy (`http://127.0.0.1:port`).
    fn base_url(&self) -> String;
    /// `Some`, gdy proces się zakończył.
    fn exited(&self) -> Option<ExitInfo>;
    /// Zabija proces.
    fn kill(&self);
}

/// Uruchamianie sidecarów (produkcja: proces; testy: serwer w procesie).
#[async_trait]
pub trait SidecarLauncher: Send + Sync {
    /// Uruchamia sidecar.
    async fn launch(&self, spec: &LaunchSpec) -> Result<Box<dyn Sidecar>, SttError>;
}

/// Uruchamianie procesu `whisper-server` (bez okna konsoli; zabijany przy porzuceniu).
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessLauncher;

struct ProcessSidecar {
    port: u16,
    child: Mutex<tokio::process::Child>,
    stderr: Arc<Mutex<VecDeque<String>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Sidecar for ProcessSidecar {
    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn exited(&self) -> Option<ExitInfo> {
        let status = lock(&self.child).try_wait().ok().flatten()?;
        let device_lost = lock(&self.stderr)
            .iter()
            .any(|l| DEVICE_LOST_MARKERS.iter().any(|m| l.contains(m)));
        Some(ExitInfo {
            code: status.code(),
            device_lost,
        })
    }

    fn kill(&self) {
        let _ = lock(&self.child).start_kill();
    }
}

#[async_trait]
impl SidecarLauncher for ProcessLauncher {
    async fn launch(&self, spec: &LaunchSpec) -> Result<Box<dyn Sidecar>, SttError> {
        let mut cmd = tokio::process::Command::new(&spec.program);
        cmd.args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = cmd
            .spawn()
            .map_err(|e| SttError::Sidecar(format!("{}: {e}", spec.program.display())))?;
        let stderr: Arc<Mutex<VecDeque<String>>> = Arc::default();
        if let Some(err) = child.stderr.take() {
            let tail = Arc::clone(&stderr);
            tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    log_server_line(&line);
                    let mut t = lock(&tail);
                    if t.len() >= 64 {
                        t.pop_front();
                    }
                    t.push_back(line);
                }
            });
        }
        Ok(Box::new(ProcessSidecar {
            port: spec.port,
            child: Mutex::new(child),
            stderr,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_per_backend() {
        let bins = SidecarBinaries {
            vulkan: Some("wv.exe".into()),
            cuda: None,
            cpu: "wc.exe".into(),
        };
        let cfg = WhisperServerConfig::new(bins.clone(), "ggml-large-v3-turbo-q5_0.bin".into());
        let cpu = build_args(&cfg, Backend::Cpu, 5555);
        assert!(cpu.contains(&"-ng".to_owned()));
        assert!(cpu.windows(2).any(|w| w == ["--port", "5555"]));
        assert!(cpu.windows(2).any(|w| w == ["--host", "127.0.0.1"]));
        let gpu = build_args(&cfg, Backend::Vulkan, 1);
        assert!(!gpu.contains(&"-ng".to_owned()) && gpu.contains(&"-fa".to_owned()));
        assert_eq!(bins.for_backend(Backend::Cuda), None);
        assert_eq!(
            bins.for_backend(Backend::Cpu),
            Some(&PathBuf::from("wc.exe"))
        );
        assert!(free_port().unwrap() > 0);
    }

    #[tokio::test]
    async fn missing_binary_is_an_error() {
        let spec = LaunchSpec {
            program: "/nie/ma/whisper-server".into(),
            args: vec![],
            port: 1,
            backend: Backend::Cpu,
        };
        assert!(ProcessLauncher.launch(&spec).await.is_err());
    }
}

/// Poziom wiersza stderr `whisper-server` w dzienniku Alfy: błędy i utrata urządzenia — `warn`,
/// model i backend (CUDA/Vulkan/CPU) — `info` (oba widoczne domyślnie), reszta — `debug` pod osobnym
/// celem `whisper_server` (tylko po jawnym włączeniu). Wiersze z transkrypcją (`[… --> …]`) nigdy
/// nie idą wyżej niż `debug` — treść rozmowy nie trafia do dziennika.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineLevel {
    Warn,
    Info,
    Debug,
}

const ERROR_MARKERS: [&str; 5] = ["error", "failed", "out of memory", "abort", "fatal"];

const INFO_MARKERS: [&str; 7] = [
    "whisper_init_from_file",
    "whisper_model_load: type",
    "model size",
    "whisper_backend_init",
    "ggml_cuda_init",
    "using device",
    "listening",
];

pub(crate) fn classify_line(line: &str) -> LineLevel {
    let trimmed = line.trim_start();
    if trimmed.starts_with('[') && trimmed.contains("-->") {
        return LineLevel::Debug;
    }
    let lower = line.to_ascii_lowercase();
    if ERROR_MARKERS.iter().any(|m| lower.contains(m))
        || DEVICE_LOST_MARKERS.iter().any(|m| line.contains(m))
    {
        LineLevel::Warn
    } else if INFO_MARKERS.iter().any(|m| line.contains(m)) {
        LineLevel::Info
    } else {
        LineLevel::Debug
    }
}

fn log_server_line(line: &str) {
    match classify_line(line) {
        LineLevel::Warn => tracing::warn!(target: "voice_stt_impl::whisper_server", "{line}"),
        LineLevel::Info => tracing::info!(target: "voice_stt_impl::whisper_server", "{line}"),
        LineLevel::Debug => tracing::debug!(target: "whisper_server", "{line}"),
    }
}

#[cfg(test)]
mod line_tests {
    use super::{LineLevel, classify_line};

    #[test]
    fn whisper_lines_are_classified_without_transcripts() {
        for (line, level) in [
            ("ggml_cuda_init: found 1 CUDA devices:", LineLevel::Info),
            (
                "whisper_backend_init_gpu: using CUDA0 backend",
                LineLevel::Info,
            ),
            (
                "whisper_model_load: model size    =  547.37 MB",
                LineLevel::Info,
            ),
            ("ggml_vulkan: Device lost: ErrorDeviceLost", LineLevel::Warn),
            ("CUDA error: out of memory", LineLevel::Warn),
            (
                "[00:00:00.000 --> 00:00:02.000]  To jest error w moim pliku",
                LineLevel::Debug,
            ),
            ("Received request: /inference", LineLevel::Debug),
        ] {
            assert_eq!(classify_line(line), level, "{line}");
        }
    }
}
