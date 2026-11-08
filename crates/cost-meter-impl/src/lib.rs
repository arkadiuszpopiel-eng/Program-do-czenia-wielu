//! Licznik kosztów i limitów (docs/modules/cost-meter/SPEC.md, PLAN §14.6, §5.5).
//!
//! `CostMeterService` implementuje `CostMeter` i `Module`: rejestruje koszty w dzienniku NDJSON
//! (append-only), liczy agregaty z rekordów (odtwarzane po restarcie), przelicza USD→PLN kursem
//! NBP (tabela A, raz dziennie; kurs zapasowy z konfiguracji), egzekwuje limit miesięczny
//! (wyłączalny), budżet tła i limity dostawców, publikuje zdarzenia `cost.*`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod nbp;
mod ndjson;
mod service;

use async_trait::async_trait;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError, ModuleManifest};
use cost_meter_contract::CostMeter;

pub use nbp::{HttpGet, NbpFxSource};
pub use ndjson::NdjsonLedger;
pub use service::{CostMeterService, SystemClock};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

#[async_trait]
impl Module for CostMeterService {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    /// Start nie pobiera kursu (bez sieci na ścieżce startu); gospodarz wywołuje `refresh_fx`
    /// w tle po starcie i raz dziennie.
    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut bus = self.bus.write().unwrap_or_else(|p| p.into_inner());
        if bus.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        *bus = Some(ctx.bus);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .map(|_| ())
            .ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        let started = self
            .bus
            .read()
            .map(|b| b.is_some())
            .unwrap_or_else(|p| p.into_inner().is_some());
        if !started {
            return HealthStatus::NotStarted;
        }
        let rate = self.current_rate();
        if rate.stale {
            HealthStatus::Degraded(format!("kurs nieaktualny ({:?})", rate.origin))
        } else {
            HealthStatus::Healthy
        }
    }
}
