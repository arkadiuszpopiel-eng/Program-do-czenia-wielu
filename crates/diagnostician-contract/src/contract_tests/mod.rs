//! Współdzielone testy kontraktowe Diagnosty (feature `contract-tests`) na `-impl` i `-fake`.
//! [`MiniWorld`] to mała atrapa portów (konfiguracja, pliki, restarty); pełny świat chaosowy
//! (24 awarie) żyje w `diagnostician-fake`.

mod cases;
mod kernel;

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::Event;
use serde_json::Value;

use crate::{
    Detection, Diagnostician, KernelApprovals, KernelOutcome, Proposal, RepairContext, RepairEnv,
    RepairPolicy, RepairStep,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Stan małego świata.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MiniState {
    /// Konfiguracja.
    pub config: BTreeMap<String, Value>,
    /// Istniejące pliki.
    pub files: BTreeSet<String>,
    /// Restarty modułów.
    pub restarts: Vec<String>,
}

/// Mała atrapa portów: `RepairEnv` + `RepairContext`.
#[derive(Debug, Default)]
pub struct MiniWorld {
    state: Mutex<MiniState>,
    verify_ok: Mutex<bool>,
    failing_restarts: Mutex<BTreeSet<String>>,
    now: Mutex<u64>,
}

impl MiniWorld {
    /// Świat z konfiguracją i plikami; weryfikacja domyślnie udana.
    pub fn new(config: &[(&str, Value)], files: &[&str]) -> Arc<Self> {
        let w = Self::default();
        {
            let mut st = lock(&w.state);
            st.config = config
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect();
            st.files = files.iter().map(|f| (*f).to_owned()).collect();
        }
        *lock(&w.verify_ok) = true;
        Arc::new(w)
    }

    /// Migawka stanu.
    pub fn snapshot(&self) -> MiniState {
        lock(&self.state).clone()
    }

    /// Wynik sondy po naprawie.
    pub fn set_verify(&self, ok: bool) {
        *lock(&self.verify_ok) = ok;
    }

    /// Restart modułu kończy się błędem.
    pub fn fail_restart(&self, module: &str) {
        lock(&self.failing_restarts).insert(module.to_owned());
    }

    /// Zmiana konfiguracji „z zewnątrz” (użytkownik).
    pub fn set_config(&self, key: &str, value: Value) {
        lock(&self.state).config.insert(key.to_owned(), value);
    }
}

#[async_trait]
impl RepairEnv for MiniWorld {
    async fn apply(&self, step: &RepairStep) -> Result<RepairStep, String> {
        let mut st = lock(&self.state);
        match step {
            RepairStep::SetConfig { key, old, new } => {
                let current = st.config.get(key).cloned();
                if current != *old {
                    return Err(format!("konflikt `{key}`"));
                }
                match new {
                    Some(v) => st.config.insert(key.clone(), v.clone()),
                    None => st.config.remove(key),
                };
                Ok(step.clone())
            }
            RepairStep::MoveFile { from, to } => {
                if !st.files.remove(from) || st.files.contains(to) {
                    return Err(format!("nie można przenieść {from}"));
                }
                st.files.insert(to.clone());
                Ok(step.clone())
            }
            RepairStep::CopyFile { from, to } => {
                if !st.files.contains(from) || st.files.contains(to) {
                    return Err(format!("nie można skopiować {from}"));
                }
                st.files.insert(to.clone());
                Ok(step.clone())
            }
            RepairStep::RemoveCopy { path, source } => {
                if !st.files.contains(source) || !st.files.remove(path) {
                    return Err(format!("nie można usunąć kopii {path}"));
                }
                Ok(step.clone())
            }
            RepairStep::RestartModule { module } => {
                if lock(&self.failing_restarts).contains(module) {
                    return Err(format!("{module} nie wstaje"));
                }
                st.restarts.push(module.clone());
                Ok(step.clone())
            }
            other => Ok(other.clone()),
        }
    }

    async fn verify(&self, _: &Detection) -> Result<bool, String> {
        Ok(*lock(&self.verify_ok))
    }
}

impl RepairContext for MiniWorld {
    fn now_ms(&self) -> u64 {
        *lock(&self.now)
    }
    fn config(&self, key: &str) -> Option<Value> {
        lock(&self.state).config.get(key).cloned()
    }
    fn current_revision(&self) -> Option<String> {
        None
    }
    fn last_good_revision(&self) -> Option<String> {
        None
    }
    fn latest_backup(&self, path: &str) -> Option<String> {
        let backup = format!("kopie/{path}");
        lock(&self.state).files.contains(&backup).then_some(backup)
    }
    fn quarantine_path(&self, path: &str) -> String {
        format!("kwarantanna/{path}")
    }
    fn free_port(&self, near: u16) -> Option<u16> {
        near.checked_add(1)
    }
    fn fallback_model(&self, _: &str) -> Option<String> {
        Some("model-zapasowy".into())
    }
    fn fallback_dir(&self, _: &str) -> Option<String> {
        Some("zapasowy".into())
    }
    fn archive_path(&self, store: &str) -> String {
        format!("archiwum/{store}")
    }
    fn reclaimable(&self, _: &str) -> Vec<(String, String)> {
        Vec::new()
    }
    fn is_kernel_path(&self, path: &str) -> bool {
        path.starts_with("jadro/")
    }
}

/// Zachowanie atrapy Brokera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerMode {
    /// Zatwierdza i wykonuje na świecie.
    Approve,
    /// Czeka w Broker-UI.
    Pending,
    /// Odmawia.
    Deny,
}

/// Atrapa Brokera: po „fizycznym potwierdzeniu” wykonuje kroki na świecie.
pub struct ScriptedBroker {
    world: Arc<MiniWorld>,
    mode: Mutex<BrokerMode>,
    seen: Mutex<Vec<Proposal>>,
}

impl ScriptedBroker {
    /// Broker nad światem.
    pub fn new(world: Arc<MiniWorld>, mode: BrokerMode) -> Arc<Self> {
        Arc::new(Self {
            world,
            mode: Mutex::new(mode),
            seen: Mutex::new(Vec::new()),
        })
    }

    /// Zmienia tryb.
    pub fn set_mode(&self, mode: BrokerMode) {
        *lock(&self.mode) = mode;
    }

    /// Propozycje przedstawione Brokerowi.
    pub fn seen(&self) -> Vec<Proposal> {
        lock(&self.seen).clone()
    }
}

#[async_trait]
impl KernelApprovals for ScriptedBroker {
    async fn execute(&self, proposal: &Proposal) -> KernelOutcome {
        lock(&self.seen).push(proposal.clone());
        let mode = *lock(&self.mode);
        match mode {
            BrokerMode::Pending => KernelOutcome::Pending {
                ticket: "B-1".into(),
            },
            BrokerMode::Deny => KernelOutcome::Denied {
                reason: "odmowa w Broker-UI".into(),
            },
            BrokerMode::Approve => {
                let mut receipts = Vec::new();
                for s in &proposal.steps {
                    match self.world.apply(s).await {
                        Ok(r) => receipts.push(r),
                        Err(reason) => return KernelOutcome::Denied { reason },
                    }
                }
                KernelOutcome::Executed { receipts }
            }
        }
    }

    async fn undo(&self, _: &Proposal, receipts: &[RepairStep]) -> KernelOutcome {
        let mut done = Vec::new();
        for s in receipts.iter().rev() {
            if let Ok(r) = self.world.apply(&s.inverse()).await {
                done.push(r);
            }
        }
        KernelOutcome::Executed { receipts: done }
    }
}

/// Parametry budowy Diagnosty w teście.
pub struct DiagSetup {
    /// Świat (środowisko i kontekst).
    pub world: Arc<MiniWorld>,
    /// Broker.
    pub broker: Arc<ScriptedBroker>,
    /// Polityka.
    pub policy: RepairPolicy,
}

/// Diagnosta zbudowany przez wywołującego.
pub struct DiagHarness {
    /// Diagnosta.
    pub diag: Arc<dyn Diagnostician>,
    /// Przesunięcie zegara (ms).
    pub advance: Arc<dyn Fn(u64) + Send + Sync>,
    /// Wyemitowane zdarzenia.
    pub events: Arc<dyn Fn() -> Vec<Event> + Send + Sync>,
}

/// Uruchamia testy kontraktowe.
pub async fn run_all<F, Fut>(factory: F)
where
    F: Fn(DiagSetup) -> Fut,
    Fut: Future<Output = DiagHarness>,
{
    cases::auto_repair_verify_and_undo(&factory).await;
    cases::consent_and_autonomy(&factory).await;
    cases::failures_roll_back(&factory).await;
    kernel::kernel_area_only_through_broker(&factory).await;
    kernel::forbidden_keys_and_conflicts(&factory).await;
    kernel::report_and_journal(&factory).await;
}
