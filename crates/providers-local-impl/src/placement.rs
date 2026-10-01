//! Umiejscowienie modelu: dzierżawa `model-residency`, backend i warstwy GPU z profilu urządzenia.

use model_residency_contract::{
    Device, Lease, LeaseId, LeaseRequest, ModelRole, Placement, Priority,
};

use crate::config::{BackendKey, GpuLayers, LaunchPlan, layers_for};
use crate::error::LocalError;
use crate::manifest::ModelEntry;
use crate::sidecar::{RESIDENCY_OWNER, Sidecar, ms};

impl Sidecar {
    pub(crate) fn acquire(
        &self,
        entry: &ModelEntry,
        placement: Placement,
    ) -> Result<Option<Lease>, LocalError> {
        let Some(res) = &self.residency else {
            return Ok(None);
        };
        let req = LeaseRequest {
            owner: RESIDENCY_OWNER.into(),
            model: entry.id.clone(),
            role: ModelRole::Llm,
            priority: Priority::Conversation,
            placement,
            vram_mb: entry.vram_mb,
            ram_mb: 512,
            cpu_ram_mb: entry.ram_mb,
            idle_unload_ms: ms(self.config.idle_unload),
        };
        res.acquire(req)
            .map(|g| Some(g.lease))
            .map_err(|e| LocalError::Residency(e.to_string()))
    }

    pub(crate) fn release_lease(&self, lease: Option<LeaseId>) {
        if let (Some(res), Some(id)) = (&self.residency, lease) {
            // Dzierżawa mogła zostać już odebrana — wtedy nie ma czego zwalniać.
            let _ = res.release(id);
        }
    }

    pub(crate) fn cpu_plan(&self, gpu: &LaunchPlan) -> LaunchPlan {
        let backend = BackendKey::Cpu;
        LaunchPlan {
            program: self
                .config
                .server_bin
                .get(&backend)
                .cloned()
                .unwrap_or_else(|| gpu.program.clone()),
            backend,
            gpu_layers: 0,
            ..gpu.clone()
        }
    }

    /// Plan: backend z profilu urządzenia, warstwy GPU z dzierżawy/budżetu, kontekst, wątki.
    pub(crate) fn plan(
        &self,
        entry: &ModelEntry,
    ) -> Result<(LaunchPlan, Option<LeaseId>), LocalError> {
        let rec = self.device.as_ref().map(|d| d.recommend());
        let threads = self.config.threads.unwrap_or_else(|| {
            self.device
                .as_ref()
                .map_or(4, |d| d.current().cpu.physical_cores.max(1))
        });
        let mut backend = rec
            .as_ref()
            .map_or(BackendKey::Cpu, |r| self.config.backend_for(r));
        let placement = if backend == BackendKey::Cpu {
            Placement::CpuOnly
        } else {
            Placement::GpuPreferred
        };
        let lease = self.acquire(entry, placement)?;
        let budget = self
            .config
            .max_vram_mb
            .or_else(|| rec.as_ref().map(|r| r.residency.vram_mb))
            .unwrap_or(0);
        let gpu_layers = match (&lease, backend) {
            (_, BackendKey::Cpu) => 0,
            (Some(l), _) if l.device == Device::Cpu => 0,
            (_, _) => match self.config.gpu_layers {
                GpuLayers::Fixed(n) => n,
                GpuLayers::Auto if lease.is_some() => entry.layers,
                GpuLayers::Auto => layers_for(entry, budget),
            },
        };
        if gpu_layers == 0 {
            backend = BackendKey::Cpu;
        }
        let program = self
            .config
            .server_bin
            .get(&backend)
            .cloned()
            .ok_or_else(|| LocalError::NoServerBinary(backend.as_str().into()))?;
        let plan = LaunchPlan {
            program,
            backend,
            gpu_layers,
            ctx: self.config.ctx.min(entry.ctx),
            threads,
        };
        Ok((plan, lease.map(|l| l.id)))
    }
}
