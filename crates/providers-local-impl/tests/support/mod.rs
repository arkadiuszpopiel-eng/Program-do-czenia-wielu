//! Narzędzia testów: fałszywy `llama-server` (binarny cel crate'a), model testowy, dostawca.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use device_profile_contract::DeviceProfile;
use lib_openai_compat::Timeouts;
use model_residency_contract::Residency;
use providers_local_impl::{
    LocalConfig, LocalProvider, ModelEntry, Sidecar, TokioLauncher, hash_path,
};

pub const MODEL: &str = "test-4b-q4_k_m";

/// Ścieżka fałszywego serwera zbudowanego przez Cargo.
pub fn fake_server() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake-llama-server"))
}

pub fn entry(url: &str) -> ModelEntry {
    ModelEntry {
        id: MODEL.into(),
        name: "Test 4B".into(),
        url: url.into(),
        file: "test-4b.Q4_K_M.gguf".into(),
        size_mb: 1,
        sha256: String::new(),
        params_b: 4.0,
        quant: "Q4_K_M".into(),
        layers: 32,
        ctx: 8_192,
        vram_mb: 3_000,
        ram_mb: 3_500,
        tools: true,
        license: "test".into(),
    }
}

/// „Instaluje" model: plik + zapisany hash (jak po pobraniu).
pub fn install(dir: &Path, entry: &ModelEntry) {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(&entry.file);
    std::fs::write(&path, b"GGUF-fake").unwrap();
    std::fs::write(hash_path(&path), format!("{}\n", "a".repeat(64))).unwrap();
}

pub struct Env {
    pub dir: tempfile::TempDir,
    pub args_file: PathBuf,
    pub log_file: PathBuf,
}

impl Env {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let args_file = dir.path().join("args.txt");
        let log_file = dir.path().join("requests.ndjson");
        Self {
            dir,
            args_file,
            log_file,
        }
    }

    pub fn models(&self) -> PathBuf {
        self.dir.path().join("models")
    }

    /// Kolejne uruchomienia: listy argumentów.
    pub fn launches(&self) -> Vec<Vec<String>> {
        std::fs::read_to_string(&self.args_file)
            .unwrap_or_default()
            .split("---")
            .map(|block| {
                block
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty())
            .collect()
    }

    /// Ciała żądań czatu, które dotarły do serwera.
    pub fn requests(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log_file)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    }

    pub fn launcher(&self, mode: &str, scenario: Option<&str>) -> TokioLauncher {
        let mut env = vec![
            (
                "FAKE_LLAMA_ARGS_FILE".to_owned(),
                self.args_file.display().to_string(),
            ),
            (
                "FAKE_LLAMA_LOG".to_owned(),
                self.log_file.display().to_string(),
            ),
            ("FAKE_LLAMA_MODE".to_owned(), mode.to_owned()),
        ];
        if let Some(s) = scenario {
            env.push(("FAKE_LLAMA_SCENARIO".to_owned(), s.to_owned()));
        }
        TokioLauncher { env }
    }

    pub fn config(&self) -> LocalConfig {
        let mut c = LocalConfig::new(self.models(), fake_server());
        c.default_model = MODEL.into();
        c.startup_timeout = Duration::from_secs(10);
        c.timeouts = Timeouts {
            connect: Duration::from_secs(1),
            first_token: Duration::from_millis(600),
            idle: Duration::from_millis(600),
        };
        c
    }

    pub fn provider_with(
        &self,
        config: LocalConfig,
        launcher: TokioLauncher,
        device: Option<Arc<dyn DeviceProfile>>,
        residency: Option<Arc<dyn Residency>>,
    ) -> LocalProvider {
        let e = entry("https://example.invalid/test.gguf");
        install(&self.models(), &e);
        let sidecar = Sidecar::new(config, Arc::new(launcher), device, residency).unwrap();
        LocalProvider::new(vec![e], sidecar)
    }

    pub fn provider(&self, mode: &str, scenario: Option<&str>) -> LocalProvider {
        self.provider_with(self.config(), self.launcher(mode, scenario), None, None)
    }
}

pub fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

/// Budżet czasowy (crates/README.md „Testy budżetów czasowych"): ściśle przy `ALFA_PERF_BUDGETS=1`,
/// na współdzielonym CI próg bezpieczeństwa ×10.
pub fn budget(strict: std::time::Duration) -> std::time::Duration {
    if std::env::var_os("ALFA_PERF_BUDGETS").is_some() {
        strict
    } else {
        strict * 10
    }
}
