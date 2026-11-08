//! Wspólne: budowa usługi nad światem testowym.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use diagnostician_contract::{KernelApprovals, RepairContext, RepairEnv, RepairPolicy};
use diagnostician_impl::DiagnosticianService;
use watchdog_contract::ManualClock;

/// Usługa z dziennikiem w pliku.
pub async fn service(
    env: Arc<dyn RepairEnv>,
    ctx: Arc<dyn RepairContext>,
    kernel: Arc<dyn KernelApprovals>,
    policy: RepairPolicy,
    clock: Arc<ManualClock>,
    journal: Option<PathBuf>,
) -> DiagnosticianService {
    DiagnosticianService::open(env, ctx, kernel, policy, clock, journal)
        .await
        .unwrap()
}
