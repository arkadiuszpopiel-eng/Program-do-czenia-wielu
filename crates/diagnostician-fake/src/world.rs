//! Świat chaosowy: symulowany stan systemu (konfiguracja z rewizjami, pliki na woluminach,
//! magazyny wpisów, porty, zegar, sieć…) jako atrapa portów `RepairEnv`, `RepairContext`
//! i Brokera (`KernelApprovals`). Kroki Diagnosty i Brokera są rejestrowane osobno.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use diagnostician_contract::{
    ConfigChange, Detection, KernelApprovals, KernelOutcome, Proposal, RepairContext, RepairEnv,
    RepairStep,
};
use serde_json::Value;
use watchdog_contract::{Clock, ManualClock};

use crate::state::{FileInfo, WorldState, volume_of};

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Świat chaosowy.
pub struct ChaosWorld {
    pub(crate) state: Mutex<WorldState>,
    pub(crate) clock: Arc<ManualClock>,
    quarantine_seq: AtomicU64,
    env_log: Mutex<Vec<RepairStep>>,
    broker_log: Mutex<Vec<RepairStep>>,
    restarts: Mutex<Vec<String>>,
    broker_approves: Mutex<bool>,
}

impl ChaosWorld {
    /// Zdrowy system bazowy.
    pub fn baseline(clock: Arc<ManualClock>) -> Arc<Self> {
        let mut st = WorldState {
            revision: "r1".into(),
            network_up: true,
            gpu_ok: true,
            ..WorldState::default()
        };
        st.volumes.insert("C:".into(), 10_000);
        st.volumes.insert("D:".into(), 50_000);
        let files = [
            ("config/shared.toml", 1, true),
            ("kopie/config/shared.toml", 1, true),
            ("config/kernel.toml", 1, true),
            ("kopie/config/kernel.toml", 1, true),
            ("sesje/s1.db", 50, true),
            ("kopie/sesje/s1.db", 50, true),
            ("sesje/s2.db", 20, true),
            ("models/qwen-3b.gguf", 2_000, true),
            ("models/whisper-base.bin", 150, true),
            ("models/whisper-small.bin", 480, true),
            ("webview/EBWebView", 100, true),
            ("dane/inne.bin", 3_000, true),
        ];
        for (p, size_mb, valid) in files {
            st.files.insert(p.into(), FileInfo { size_mb, valid });
        }
        st.entries.insert("undo-journal".into(), 1_000);
        st.entries.insert("logs/diagnostics".into(), 100);
        st.config
            .insert("voice.tts.engine".into(), Value::from("piper"));
        st.revisions.insert("r1".into(), st.config.clone());
        st.last_good = Some("r1".into());
        Arc::new(Self {
            state: Mutex::new(st),
            clock,
            quarantine_seq: AtomicU64::new(0),
            env_log: Mutex::new(Vec::new()),
            broker_log: Mutex::new(Vec::new()),
            restarts: Mutex::new(Vec::new()),
            broker_approves: Mutex::new(true),
        })
    }

    /// Migawka stanu.
    pub fn snapshot(&self) -> WorldState {
        lock(&self.state).clone()
    }

    /// Zmiana stanu (wstrzykiwanie awarii).
    pub fn mutate(&self, f: impl FnOnce(&mut WorldState)) {
        f(&mut lock(&self.state));
    }

    /// Kroki wykonane przez port Diagnosty (`RepairEnv`).
    pub fn env_log(&self) -> Vec<RepairStep> {
        lock(&self.env_log).clone()
    }

    /// Kroki wykonane przez Brokera.
    pub fn broker_log(&self) -> Vec<RepairStep> {
        lock(&self.broker_log).clone()
    }

    /// Restarty modułów.
    pub fn restarts(&self) -> Vec<String> {
        lock(&self.restarts).clone()
    }

    /// Czy Broker zatwierdza (symulacja fizycznego potwierdzenia w Broker-UI).
    pub fn set_broker_approves(&self, approves: bool) {
        *lock(&self.broker_approves) = approves;
    }

    fn apply_logged(
        &self,
        step: &RepairStep,
        log: &Mutex<Vec<RepairStep>>,
    ) -> Result<RepairStep, String> {
        let done = lock(&self.state).apply(step)?;
        if let RepairStep::RestartModule { module } = step {
            lock(&self.restarts).push(module.clone());
        }
        lock(log).push(done.clone());
        Ok(done)
    }
}

#[async_trait]
impl RepairEnv for ChaosWorld {
    async fn apply(&self, step: &RepairStep) -> Result<RepairStep, String> {
        self.apply_logged(step, &self.env_log)
    }

    async fn verify(&self, detection: &Detection) -> Result<bool, String> {
        Ok(crate::probe::healthy(
            &self.snapshot(),
            detection,
            self.clock.now_ms(),
        ))
    }
}

impl RepairContext for ChaosWorld {
    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }
    fn config(&self, key: &str) -> Option<Value> {
        lock(&self.state).config.get(key).cloned()
    }
    fn current_revision(&self) -> Option<String> {
        Some(lock(&self.state).revision.clone())
    }
    fn last_good_revision(&self) -> Option<String> {
        lock(&self.state).last_good.clone()
    }
    fn revision_diff(&self, from: &str, to: &str) -> Option<Vec<ConfigChange>> {
        let st = lock(&self.state);
        let current = if st.revision == from {
            &st.config
        } else {
            st.revisions.get(from)?
        };
        let target = st.revisions.get(to)?;
        let keys: std::collections::BTreeSet<&String> =
            current.keys().chain(target.keys()).collect();
        Some(
            keys.into_iter()
                .filter(|k| current.get(*k) != target.get(*k))
                .map(|k| ConfigChange {
                    key: k.clone(),
                    current: current.get(k).cloned(),
                    target: target.get(k).cloned(),
                })
                .collect(),
        )
    }
    fn latest_backup(&self, path: &str) -> Option<String> {
        let backup = format!("kopie/{path}");
        lock(&self.state)
            .files
            .contains_key(&backup)
            .then_some(backup)
    }
    fn quarantine_path(&self, path: &str) -> String {
        let n = self.quarantine_seq.fetch_add(1, Ordering::SeqCst);
        format!("kwarantanna/{n}/{}", path.replace(['%', ':'], "_"))
    }
    fn free_port(&self, near: u16) -> Option<u16> {
        let st = lock(&self.state);
        (near.saturating_add(1)..=u16::MAX).find(|p| !st.ports_taken.contains(p))
    }
    fn fallback_model(&self, model: &str) -> Option<String> {
        match model {
            "bielik-4.5b" => Some("qwen-3b".into()),
            "whisper-small" => Some("whisper-base".into()),
            _ => None,
        }
    }
    fn fallback_dir(&self, _: &str) -> Option<String> {
        Some("C:/Users/ja/AppData/Local/Alfa/zapas".into())
    }
    fn archive_path(&self, store: &str) -> String {
        format!("D:/archiwum/{store}")
    }
    fn reclaimable(&self, volume: &str) -> Vec<(String, String)> {
        lock(&self.state)
            .files
            .keys()
            .filter(|p| p.starts_with("cache/") && volume_of(p) == volume)
            .map(|p| (p.clone(), format!("D:/{p}")))
            .collect()
    }
    fn is_kernel_path(&self, path: &str) -> bool {
        [
            "%LOCALAPPDATA%/Alfa/versions",
            "config/kernel",
            "kopie/config/kernel",
            "broker/",
        ]
        .iter()
        .any(|p| path.starts_with(p))
    }
}

/// Atrapa Brokera nad światem: wykonuje kroki po „fizycznym potwierdzeniu”.
pub struct ChaosBroker(pub Arc<ChaosWorld>);

#[async_trait]
impl KernelApprovals for ChaosBroker {
    async fn execute(&self, proposal: &Proposal) -> KernelOutcome {
        if !*lock(&self.0.broker_approves) {
            return KernelOutcome::Denied {
                reason: "odmowa w Broker-UI".into(),
            };
        }
        let mut receipts = Vec::new();
        for step in &proposal.steps {
            match self.0.apply_logged(step, &self.0.broker_log) {
                Ok(r) => receipts.push(r),
                Err(reason) => return KernelOutcome::Denied { reason },
            }
        }
        KernelOutcome::Executed { receipts }
    }

    async fn undo(&self, _: &Proposal, receipts: &[RepairStep]) -> KernelOutcome {
        let mut done = Vec::new();
        for step in receipts.iter().rev() {
            if let Ok(r) = self.0.apply_logged(&step.inverse(), &self.0.broker_log) {
                done.push(r);
            }
        }
        KernelOutcome::Executed { receipts: done }
    }
}
