//! Dzierżawa RAM w `model-residency`: embedder na CPU, priorytet rozmowy (zapytania recall są
//! interaktywne), zwolnienie po bezczynności; odebranie dzierżawy → wyładowanie modelu w wątku tła.

use std::sync::Arc;
use std::sync::mpsc::Sender;

use model_residency_contract::{
    Lease, LeaseId, LeaseListener, LeaseRequest, ModelRole, Placement, Priority, Residency,
    Revocation,
};

use crate::error::EmbedError;
use crate::manifest::EmbedManifest;
use crate::worker::Job;

/// Właściciel dzierżaw embeddera w `model-residency`.
pub const RESIDENCY_OWNER: &str = "search-embedder";

/// Żądanie dzierżawy dla modelu z manifestu (RAM z `ram_mb`, bez VRAM).
pub fn lease_request(manifest: &EmbedManifest) -> LeaseRequest {
    LeaseRequest {
        owner: RESIDENCY_OWNER.into(),
        model: manifest.model_id(),
        role: ModelRole::Embedder,
        priority: Priority::Conversation,
        placement: Placement::CpuOnly,
        vram_mb: 0,
        ram_mb: 0,
        cpu_ram_mb: manifest.ram_mb,
        idle_unload_ms: manifest.idle_unload_s.saturating_mul(1000),
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
    ) -> Result<Self, EmbedError> {
        let grant = residency
            .acquire(request)
            .map_err(|e| EmbedError::Residency(e.to_string()))?;
        Ok(Self {
            residency,
            id: grant.lease.id,
        })
    }

    /// Odświeża licznik bezczynności.
    pub fn touch(&self) {
        let _ = self.residency.touch(self.id);
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

/// Słuchacz właściciela: odebranie dzierżawy → zlecenie wyładowania (nie blokuje zarządcy).
pub struct UnloadOnRevoke {
    jobs: Sender<Job>,
}

impl UnloadOnRevoke {
    /// Słuchacz wysyłający zlecenia do wątku embeddera.
    pub fn new(jobs: Sender<Job>) -> Self {
        Self { jobs }
    }
}

impl LeaseListener for UnloadOnRevoke {
    fn revoked(&self, revocation: &Revocation) {
        let _ = self.jobs.send(Job::Revoked(revocation.lease.id));
    }

    fn moved(&self, _lease: &Lease) {
        // Embedder działa wyłącznie na CPU — przeniesienie go nie dotyczy.
    }
}
