//! Wspólne pomocniki testów `safety-broker-impl`.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use compliance_contract::PathEnv;
use core_bus_fake::FakeBus;
use platform_contract::{PlatformError, ProcessHandle, ProcessPort, ProcessSpec, ProcessStatus};
use safety_broker_contract::KernelPolicy;
use safety_broker_impl::audit::{AuditSink, MemoryAudit};
use safety_broker_impl::{BrokerConfig, BrokerEngine, KeyMode};
use watchdog_contract::ManualClock;

/// Port procesów przyjmujący każdy uchwyt i zapamiętujący zabite (bez prawdziwych procesów).
#[derive(Debug, Default)]
pub struct RecordingProcesses {
    pub killed: Mutex<BTreeSet<u32>>,
}

impl ProcessPort for RecordingProcesses {
    fn spawn(&self, _: ProcessSpec) -> Result<ProcessHandle, PlatformError> {
        Err(PlatformError::Unsupported("test".into()))
    }
    fn kill_tree(&self, h: ProcessHandle) -> Result<(), PlatformError> {
        self.killed.lock().unwrap().insert(h.0);
        Ok(())
    }
    fn status(&self, _: ProcessHandle) -> Result<ProcessStatus, PlatformError> {
        Ok(ProcessStatus::Running)
    }
    fn foreground_is_elevated(&self) -> bool {
        false
    }
}

/// Środowisko profilu testowego.
pub fn env() -> PathEnv {
    PathEnv::windows_profile(r"C:\Users\ala")
}

/// Broker z danym Audytem.
pub fn engine_with(
    policy: KernelPolicy,
    clock: Arc<ManualClock>,
    audit: Arc<dyn AuditSink>,
) -> BrokerEngine {
    let config = BrokerConfig {
        policy,
        env: env(),
        key_mode: KeyMode::Random,
    };
    BrokerEngine::new(
        config,
        clock,
        audit,
        Arc::new(RecordingProcesses::default()),
    )
    .unwrap()
    .with_bus(Arc::new(FakeBus::default()))
}

/// Broker z Audytem w pamięci (zwraca też Audyt i zegar).
pub fn engine() -> (BrokerEngine, Arc<MemoryAudit>, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let e = engine_with(
        safety_broker_contract::contract_tests::test_policy(),
        clock.clone(),
        audit.clone(),
    );
    (e, audit, clock)
}
