//! Dzierżawa w `model-residency` (mały model na CPU, rezydentny gdy głos aktywny).

use std::sync::Arc;

use model_residency_contract::{
    LeaseId, LeaseRequest, ModelRole, Placement, Priority, Residency, ResidencyError,
};

/// Szacunek RAM modelu Silero + tract (MB).
pub const SILERO_RAM_MB: u32 = 30;

/// Żądanie dzierżawy dla modelu VAD.
pub fn vad_lease_request(model: &str) -> LeaseRequest {
    LeaseRequest {
        owner: "voice-vad".into(),
        model: model.into(),
        role: ModelRole::Vad,
        priority: Priority::VoiceRt,
        placement: Placement::CpuOnly,
        vram_mb: 0,
        ram_mb: 0,
        cpu_ram_mb: SILERO_RAM_MB,
        idle_unload_ms: 0,
    }
}

/// Dzierżawa zwalniana przy `drop`.
pub struct LeaseGuard {
    residency: Arc<dyn Residency>,
    id: LeaseId,
}

impl std::fmt::Debug for LeaseGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LeaseGuard").field("id", &self.id).finish()
    }
}

impl LeaseGuard {
    /// Przydziela dzierżawę.
    pub fn acquire(
        residency: Arc<dyn Residency>,
        request: LeaseRequest,
    ) -> Result<Self, ResidencyError> {
        let grant = residency.acquire(request)?;
        Ok(Self {
            residency,
            id: grant.lease.id,
        })
    }

    /// Identyfikator.
    pub fn id(&self) -> LeaseId {
        self.id
    }
}

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        let _ = self.residency.release(self.id);
    }
}
