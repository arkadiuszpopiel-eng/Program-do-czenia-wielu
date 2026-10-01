//! Implementacja Marszałka (docs/modules/marshal/SPEC.md, PLAN §9.4).
//!
//! Rdzeń (`MarshalCore`: księga reguł, sprawdzanie „tylko zawęża”, nadzór) pochodzi
//! z kontraktu; ten crate dostarcza: subskrypcję `scheduler.*`/`triggers.*` z magistrali (nadzór),
//! przegląd długich blokad co minutę, raport dzienny wg crona w strefie nadzoru (domyślnie
//! 21:00 Europe/Warsaw), publikację `marshal.*`, księgę reguł w pliku ([`FileMarshalStore`])
//! i port tłumacza poleceń (LLM przez Router — podpina `app-*`; bez niego [`NoTranslator`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod service;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::NaiveDate;
use core_bus_contract::Event;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use marshal_contract::{
    Approver, Ceiling, DailyReport, EffectivePolicy, Escalation, Marshal, MarshalError, Proposal,
    Rule, RuleId, RuleTranslator, WatchConfig,
};
use triggers_contract::CronExpr;

pub use service::{
    CHECK_EVERY_MS, FileMarshalStore, ImplHost, MarshalStore, MemMarshalStore, NoTranslator,
};

use crate::service::{Service, Settings, lock};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Domyślna pora raportu dziennego.
pub const DEFAULT_REPORT_CRON: &str = "0 21 * * *";

/// Moduł Marszałka.
pub struct MarshalModule {
    manifest: ModuleManifest,
    translator: Arc<dyn RuleTranslator>,
    store: Arc<dyn MarshalStore>,
    settings: Settings,
    running: Mutex<Option<Arc<Service>>>,
}

impl MarshalModule {
    /// Nowy moduł: tłumacz poleceń, magazyn księgi.
    pub fn new(
        translator: Arc<dyn RuleTranslator>,
        store: Arc<dyn MarshalStore>,
    ) -> Result<Self, ManifestError> {
        let report = CronExpr::parse(DEFAULT_REPORT_CRON)
            .map_err(|e| ManifestError::Syntax(e.to_string()))?;
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            translator,
            store,
            settings: Settings {
                watch: WatchConfig::default(),
                report,
                start_ms: None,
            },
            running: Mutex::new(None),
        })
    }

    /// Konfiguracja nadzoru i pory raportu (`[marshal]` w TOML).
    #[must_use]
    pub fn with_settings(mut self, watch: WatchConfig, report: CronExpr) -> Self {
        self.settings.watch = watch;
        self.settings.report = report;
        self
    }

    /// Zegar od zadanej chwili (testy).
    #[must_use]
    pub fn with_start_ms(mut self, start_ms: u64) -> Self {
        self.settings.start_ms = Some(start_ms);
        self
    }

    fn svc(&self) -> Result<Arc<Service>, MarshalError> {
        lock(&self.running).clone().ok_or(MarshalError::NotStarted)
    }

    /// Bieżący czas modułu (ms UTC).
    pub fn now_ms(&self) -> Option<u64> {
        use marshal_contract::MarshalHost;
        self.svc().ok().map(|s| s.core.host().now_ms())
    }
}

#[async_trait]
impl Marshal for MarshalModule {
    async fn propose(&self, text: &str) -> Result<Proposal, MarshalError> {
        let svc = self.svc()?;
        svc.core.propose(text).await
    }

    fn propose_drafts(&self, text: &str, drafts: Vec<serde_json::Value>) -> Proposal {
        match self.svc() {
            Ok(s) => s.core.propose_drafts(text, drafts),
            Err(_) => marshal_contract::RuleBook::default().propose(text, Vec::new(), 0),
        }
    }

    fn approve(&self, proposal: u64, approver: Approver) -> Result<Vec<Rule>, MarshalError> {
        self.svc()?.core.approve(proposal, approver)
    }

    fn reject(&self, proposal: u64) -> Result<(), MarshalError> {
        self.svc()?.core.reject(proposal)
    }

    fn revoke(&self, rule: &RuleId, approver: Approver) -> Result<Rule, MarshalError> {
        self.svc()?.core.revoke(rule, approver)
    }

    fn rules(&self) -> Vec<Rule> {
        self.svc().map(|s| s.core.rules()).unwrap_or_default()
    }

    fn effective(&self) -> EffectivePolicy {
        match self.svc() {
            Ok(s) => s.core.effective(),
            Err(_) => marshal_contract::compose(&Ceiling::default(), &[]),
        }
    }

    fn set_ceiling(&self, ceiling: Ceiling) {
        if let Ok(s) = self.svc() {
            s.core.set_ceiling(ceiling);
        }
    }

    fn observe(&self, event: &Event) -> Vec<Escalation> {
        self.svc()
            .map(|s| s.core.observe(event))
            .unwrap_or_default()
    }

    fn check(&self) -> Vec<Escalation> {
        self.svc().map(|s| s.core.check()).unwrap_or_default()
    }

    fn daily_report(&self, day: NaiveDate) -> DailyReport {
        self.svc()
            .map(|s| s.core.daily_report(day))
            .unwrap_or_default()
    }
}

#[async_trait]
impl Module for MarshalModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        if lock(&self.running).is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        let svc = Service::start(
            ctx.bus,
            Arc::clone(&self.translator),
            Arc::clone(&self.store),
            self.settings.clone(),
        )
        .await
        .map_err(ModuleError::Other)?;
        *lock(&self.running) = Some(svc);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        let svc = lock(&self.running).take().ok_or(ModuleError::NotStarted)?;
        svc.shutdown();
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        match self.running.try_lock() {
            Ok(guard) if guard.is_some() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::NotStarted,
            Err(_) => HealthStatus::Degraded("stan zajęty".into()),
        }
    }
}
