//! Implementacja klasyfikatora ryzyka (docs/modules/risk-classifier/SPEC.md).
//!
//! Logika decyzyjna to czysta tabela z `risk-classifier-contract` ([`evaluate`]); ten crate
//! dokłada progi polityki Jądra (zmiana wyłącznie z dowodem Brokera [`KernelAuthority`]),
//! zdarzenia `risk.*` na magistrali i integrację z rejestrem modułów.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use async_trait::async_trait;
use compliance_contract::KernelAuthority;
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
use risk_classifier_contract::{
    ActionFacts, AutonomyLevel, EVENT_RISK_CLASSIFIED, EVENT_RULES_CHANGED,
    EVENT_TRIFECTA_DETECTED, RiskClassifier, RiskPolicy, RiskVerdict, Verdict, evaluate,
    event_kind,
};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Błędy klasyfikatora.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClassifierError {
    /// Niepoprawne progi.
    #[error("niepoprawna polityka ryzyka: {0}")]
    InvalidPolicy(String),
    /// Zepsuty manifest.
    #[error("manifest: {0}")]
    Manifest(String),
}

/// Klasyfikator tabelaryczny z progami polityki Jądra.
pub struct TableClassifier {
    manifest: ModuleManifest,
    policy: RwLock<RiskPolicy>,
    bus: Mutex<Option<Arc<dyn EventBus>>>,
}

impl TableClassifier {
    /// Tworzy klasyfikator z progami (walidowanymi).
    pub fn new(policy: RiskPolicy) -> Result<Self, ClassifierError> {
        policy.validate().map_err(ClassifierError::InvalidPolicy)?;
        let manifest = ModuleManifest::parse_toml(MODULE_TOML)
            .map_err(|e: ManifestError| ClassifierError::Manifest(e.to_string()))?;
        Ok(Self {
            manifest,
            policy: RwLock::new(policy),
            bus: Mutex::new(None),
        })
    }

    /// Zmiana progów — wyłącznie z dowodem uprawnień Brokera; publikuje `risk.rules.changed`.
    pub async fn set_policy(
        &self,
        _authority: &KernelAuthority,
        policy: RiskPolicy,
    ) -> Result<(), ClassifierError> {
        policy.validate().map_err(ClassifierError::InvalidPolicy)?;
        let old = {
            let mut guard = self.policy.write().unwrap_or_else(|p| p.into_inner());
            std::mem::replace(&mut *guard, policy)
        };
        let payload = serde_json::json!({ "old": old, "new": policy });
        self.publish(EVENT_RULES_CHANGED, Level::Audit, payload)
            .await;
        Ok(())
    }

    /// Werdykt + zdarzenia diagnostyczne (`risk.classified` przy `Ask`/`HardBlock`,
    /// `risk.trifecta_detected`). Zapis do Audytu robi Broker, nie klasyfikator.
    pub async fn evaluate_reported(
        &self,
        facts: &ActionFacts,
        autonomy: AutonomyLevel,
    ) -> RiskVerdict {
        let verdict = self.evaluate(facts, autonomy);
        if verdict.verdict != Verdict::Proceed {
            let payload = serde_json::json!({
                "tool": facts.tool,
                "autonomy": autonomy,
                "level": verdict.level,
                "verdict": verdict.verdict,
                "rules": verdict.rules,
            });
            self.publish(EVENT_RISK_CLASSIFIED, Level::Info, payload)
                .await;
        }
        if facts.trifecta() {
            let payload = serde_json::json!({ "tool": facts.tool, "egress": facts.egress });
            self.publish(EVENT_TRIFECTA_DETECTED, Level::Warn, payload)
                .await;
        }
        verdict
    }

    fn bus(&self) -> MutexGuard<'_, Option<Arc<dyn EventBus>>> {
        self.bus.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn publish(&self, name: &str, level: Level, payload: serde_json::Value) {
        let bus = self.bus().clone();
        if let Some(bus) = bus {
            // Zdarzenie diagnostyczne: błąd magistrali nie zmienia werdyktu.
            let _ = bus
                .publish(Event::new(event_kind(name), level, payload))
                .await;
        }
    }
}

impl RiskClassifier for TableClassifier {
    fn policy(&self) -> RiskPolicy {
        *self.policy.read().unwrap_or_else(|p| p.into_inner())
    }

    fn evaluate(&self, facts: &ActionFacts, autonomy: AutonomyLevel) -> RiskVerdict {
        evaluate(facts, autonomy, &self.policy())
    }
}

#[async_trait]
impl Module for TableClassifier {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut bus = self.bus();
        if bus.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        *bus = Some(ctx.bus);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus().take().map(|_| ()).ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        if self.bus().is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::NotStarted
        }
    }
}
