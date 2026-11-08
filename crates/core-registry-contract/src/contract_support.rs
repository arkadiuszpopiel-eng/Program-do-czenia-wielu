//! Wsparcie testów kontraktowych rejestru (feature `contract-tests`): moduł-zaślepka,
//! budowanie manifestów testowych i środowisko `Harness`. Eksportowane przez `contract_tests`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::EventBus;

use crate::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError, ModuleManifest};

/// Wspólny dziennik startów/zatrzymań modułów-zaślepek (`"start:a"`, `"stop:a"`, `"fail:a"`).
pub type StartLog = Arc<Mutex<Vec<String>>>;

/// Moduł-zaślepka: zapisuje start/stop do dziennika; opcjonalnie zawsze odmawia startu.
pub struct StubModule {
    manifest: ModuleManifest,
    log: StartLog,
    running: bool,
    fail_start: bool,
}

impl StubModule {
    /// Zaślepka z manifestem i wspólnym dziennikiem.
    pub fn new(manifest: ModuleManifest, log: StartLog) -> Self {
        Self {
            manifest,
            log,
            running: false,
            fail_start: false,
        }
    }

    /// Zaślepka, której `start` zawsze kończy się błędem.
    pub fn failing(manifest: ModuleManifest, log: StartLog) -> Self {
        Self {
            fail_start: true,
            ..Self::new(manifest, log)
        }
    }

    fn note(&self, what: &str) {
        let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        log.push(format!("{what}:{}", self.manifest.id));
    }
}

#[async_trait]
impl Module for StubModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, _ctx: ModuleContext) -> Result<(), ModuleError> {
        if self.running {
            return Err(ModuleError::AlreadyStarted);
        }
        if self.fail_start {
            self.note("fail");
            return Err(ModuleError::Other("awaria testowa".into()));
        }
        self.running = true;
        self.note("start");
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        if !self.running {
            return Err(ModuleError::NotStarted);
        }
        self.running = false;
        self.note("stop");
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        if self.running {
            HealthStatus::Healthy
        } else {
            HealthStatus::NotStarted
        }
    }
}

/// Manifest testowy (`kind = service`, budżet minimalny).
pub fn manifest(
    id: &str,
    lifecycle: Lifecycle,
    provides: &[&str],
    requires: &[&str],
) -> ModuleManifest {
    let list = |xs: &[&str]| {
        xs.iter()
            .map(|x| format!("\"{x}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let lifecycle = match lifecycle {
        Lifecycle::Lazy => "lazy",
        Lifecycle::OnDemand => "on-demand",
        Lifecycle::Always => "always",
    };
    let text = format!(
        "id = \"{id}\"\nversion = \"1.0.0\"\nkind = \"service\"\nlifecycle = \"{lifecycle}\"\n\
         provides = [{}]\nrequires = [{}]\n[budget]\nram_mb = 1\ncpu_pct = 1\n",
        list(provides),
        list(requires)
    );
    ModuleManifest::parse_toml(&text).unwrap_or_else(|e| panic!("{e}"))
}

/// Środowisko testu: świeży rejestr, jego magistrala, limit bezczynności i przesuwanie zegara.
pub struct Harness<R> {
    /// Rejestr bez zarejestrowanych modułów.
    pub registry: R,
    /// Magistrala, na którą rejestr publikuje zdarzenia.
    pub bus: Arc<dyn EventBus>,
    /// Domyślny limit bezczynności rejestru.
    pub idle_timeout: Duration,
    /// Przesuwa zegar rejestru (wirtualny) o podany czas.
    pub advance: Box<dyn Fn(Duration) + Send + Sync>,
}
