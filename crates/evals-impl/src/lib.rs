//! Implementacja harnessu ewaluacji (docs/modules/evals/SPEC.md): katalog zestawów z dysku,
//! bramka z ukrytym holdoutem, przebiegi i raporty, moduł rejestru.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod dir;
mod holdout;

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::{EventBus, Level};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use evals_contract::{
    BootstrapConfig, CandidateRunner, Direction, EVENT_RUN_COMPLETED, EvalError, EvalReport,
    PASS_RATE, Split, SuiteCatalog, SuiteId, SuiteStatus, Variant, build_report, compare,
    evals_event, run_variant,
};
use tokio::sync::RwLock;

pub use dir::{DirCatalog, SEALED_DIRS};
pub use holdout::HoldoutGate;

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Parametry przebiegu na podziale publicznym.
#[derive(Debug, Clone)]
pub struct RunSpec {
    /// Zestaw.
    pub suite: SuiteId,
    /// Podział (`dev` albo `test`; `holdout` → [`EvalError::HoldoutSealed`]).
    pub split: Split,
    /// Powtórzenia (≥ 1).
    pub repeats: u32,
    /// Bootstrap.
    pub bootstrap: BootstrapConfig,
    /// Znacznik czasu raportu (ms).
    pub now_ms: u64,
}

/// Przebieg wariantu z raportem (integralność dołączona).
pub async fn run_suite(
    catalog: &dyn SuiteCatalog,
    spec: &RunSpec,
    runner: &dyn CandidateRunner,
    variant: &Variant,
) -> Result<EvalReport, EvalError> {
    let manifest = catalog.manifest(&spec.suite)?;
    let integrity = catalog.verify(&spec.suite)?;
    let cases = catalog.cases(&spec.suite, spec.split)?;
    let outcomes = run_variant(runner, variant, &cases, spec.repeats.max(1)).await;
    let mut report = build_report(
        &manifest,
        spec.split,
        &variant.id,
        &outcomes,
        &spec.bootstrap,
        spec.now_ms,
    );
    report.integrity = Some(integrity);
    Ok(report)
}

/// Porównanie przed/po: raport kandydata z porównaniem `pass_rate` względem bazy.
pub async fn compare_suite(
    catalog: &dyn SuiteCatalog,
    spec: &RunSpec,
    runner: &dyn CandidateRunner,
    baseline: &Variant,
    candidate: &Variant,
) -> Result<EvalReport, EvalError> {
    let manifest = catalog.manifest(&spec.suite)?;
    let integrity = catalog.verify(&spec.suite)?;
    let cases = catalog.cases(&spec.suite, spec.split)?;
    let base = run_variant(runner, baseline, &cases, spec.repeats.max(1)).await;
    let cand = run_variant(runner, candidate, &cases, spec.repeats.max(1)).await;
    let mut report = build_report(
        &manifest,
        spec.split,
        &candidate.id,
        &cand,
        &spec.bootstrap,
        spec.now_ms,
    );
    report.integrity = Some(integrity);
    report.comparison = compare(
        &base,
        &cand,
        PASS_RATE,
        Direction::HigherIsBetter,
        &spec.bootstrap,
    );
    Ok(report)
}

/// Moduł `evals` dla rejestru: zdrowie = poprawne manifesty i nienaruszone zestawy zamrożone.
pub struct EvalsModule {
    manifest: ModuleManifest,
    catalog: Arc<DirCatalog>,
    bus: RwLock<Option<Arc<dyn EventBus>>>,
}

impl EvalsModule {
    /// Moduł nad katalogiem.
    pub fn new(catalog: Arc<DirCatalog>) -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            catalog,
            bus: RwLock::new(None),
        })
    }

    /// Publikuje `evals.run.completed` z raportem (jeśli moduł uruchomiony).
    pub async fn publish_report(&self, report: &EvalReport) {
        if let Some(bus) = self.bus.read().await.clone() {
            let payload = serde_json::to_value(report).unwrap_or_default();
            let _ = bus
                .publish(evals_event(EVENT_RUN_COMPLETED, Level::Info, payload))
                .await;
        }
    }
}

#[async_trait]
impl Module for EvalsModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut bus = self.bus.write().await;
        if bus.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        *bus = Some(ctx.bus);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus
            .write()
            .await
            .take()
            .map(|_| ())
            .ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        match self.bus.try_read() {
            Ok(guard) if guard.is_none() => return HealthStatus::NotStarted,
            Ok(_) => {}
            Err(_) => return HealthStatus::Degraded("magistrala zajęta".into()),
        }
        let broken: Vec<String> = self
            .catalog
            .suites()
            .into_iter()
            .filter(|s| s.status == SuiteStatus::Frozen)
            .filter(|s| !self.catalog.verify(&s.suite).is_ok_and(|r| r.is_intact()))
            .map(|s| s.suite.to_string())
            .collect();
        if !broken.is_empty() {
            return HealthStatus::Unhealthy(format!(
                "naruszona integralność zestawów zamrożonych: {}",
                broken.join(", ")
            ));
        }
        if self.catalog.problems().is_empty() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded(format!(
                "{} problemów z manifestami",
                self.catalog.problems().len()
            ))
        }
    }
}
