//! Umiejscowienie modelu: dzierżawa `model-residency`, backend, kontekst i warstwy GPU z profilu
//! urządzenia. Dzierżawa liczy wagi z narzutem **i KV cache dla kontekstu uruchomienia** — dla
//! tylu warstw, ile trafi na kartę (częściowe odciążenie, gdy model się nie mieści). Obok LLM
//! zostaje miejsce na STT na GPU, gdy profil go tam przewiduje: najpierw mniejszy kontekst
//! ([`crate::LocalConfig::ctx_for`]), potem mniej warstw ([`crate::layers_for`]).

use device_profile_contract::{Backend, Recommendation};
use model_residency_contract::{
    Device, GpuId, Lease, LeaseId, LeaseRequest, ModelRole, Placement, Priority, card_for,
};

use crate::config::{BackendKey, GpuLayers, LaunchPlan, layers_for};
use crate::error::LocalError;
use crate::manifest::ModelEntry;
use crate::sidecar::{Sidecar, lock, ms};

impl Sidecar {
    pub(crate) fn acquire(
        &self,
        entry: &ModelEntry,
        placement: Placement,
        ctx: u32,
        gpu_layers: u32,
        gpu: Option<GpuId>,
    ) -> Result<Option<Lease>, LocalError> {
        let Some(res) = &self.residency else {
            return Ok(None);
        };
        // Karta zintegrowana nie ma własnej pamięci: model zajmuje RAM i nie obciąża budżetu
        // VRAM karty głównej (dzierżawa „na procesorze” z pełnym rozmiarem modelu).
        let shared = self.config.shared_memory;
        let req = LeaseRequest {
            owner: self.owner(),
            model: entry.id.clone(),
            role: ModelRole::Llm,
            priority: Priority::Conversation,
            placement: if shared {
                Placement::CpuOnly
            } else {
                placement
            },
            vram_mb: if shared {
                0
            } else {
                entry.vram_for(gpu_layers, ctx)
            },
            ram_mb: if shared {
                entry.ram_need(ctx)
            } else {
                entry.ram_for(gpu_layers, ctx)
            },
            cpu_ram_mb: entry.ram_need(ctx),
            idle_unload_ms: ms(self.config.idle_unload),
            // Karta zintegrowana liczona w RAM (wyżej) — bez puli karty.
            gpu: if shared { None } else { gpu },
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

    /// Dzierżawa w użyciu na czas żądań: nie jest zwalniana jako bezczynna ani wypierana przez
    /// równy priorytet; koniec ostatniego żądania odświeża licznik bezczynności zarządcy (bez tego
    /// model rozmawiający bez przerwy byłby wyładowywany co `idle_unload` od załadowania).
    pub(crate) fn mark_in_use(&self, in_use: bool) {
        let lease = *lock(&self.lease);
        if let (Some(res), Some(id)) = (&self.residency, lease) {
            // Dzierżawa mogła zostać odebrana — sidecar zatrzyma się przy najbliższej okazji.
            let _ = res.set_in_use(id, in_use);
        }
    }

    pub(crate) fn cpu_plan(&self, gpu: &LaunchPlan) -> LaunchPlan {
        let backend = BackendKey::Cpu;
        LaunchPlan {
            program: self
                .config
                .server(backend)
                .unwrap_or_else(|| gpu.program.clone()),
            backend,
            gpu_layers: 0,
            device: None,
            ..gpu.clone()
        }
    }

    /// Karta, na której policzy serwer (C6.6): przypięta z konfiguracji (tylko przy włączonym
    /// przypinaniu i karcie z tym backendem) albo główna z profilu — ta sama, którą wskazuje
    /// `--device` ([`Self::pinned_device`]); CPU — bez karty.
    fn target_gpu(&self, backend: BackendKey) -> Option<GpuId> {
        let backend = match backend {
            BackendKey::Cpu => return None,
            BackendKey::Cuda => Backend::Cuda,
            BackendKey::Vulkan => Backend::Vulkan,
        };
        let profile = self.device.as_ref()?.current();
        let name = self
            .config
            .gpu_name
            .as_deref()
            .filter(|_| self.config.pin_device);
        card_for(&profile, backend, name)
    }

    /// Budżet VRAM: limit z konfiguracji, pula karty docelowej w zarządcy rezydencji (z emulacją;
    /// karta główna albo nieznana — budżet karty głównej), rekomendacja.
    fn vram_budget(&self, rec: Option<&Recommendation>, gpu: Option<GpuId>) -> u32 {
        self.config
            .max_vram_mb
            .or_else(|| {
                self.residency.as_ref().map(|r| {
                    let state = r.snapshot();
                    gpu.and_then(|g| state.gpu(g))
                        .map_or(state.budget.vram_mb, |g| g.pool.vram_mb)
                })
            })
            .or_else(|| rec.map(|r| r.residency.vram_mb))
            .unwrap_or(0)
    }

    /// Rezerwa VRAM na STT obok LLM, gdy profil urządzenia przewiduje STT na karcie.
    fn stt_reserve(&self, rec: Option<&Recommendation>) -> u32 {
        let stt_on_gpu = rec.is_some_and(|r| r.stt_backend != Backend::Cpu);
        if stt_on_gpu {
            self.config.stt_reserve_mb
        } else {
            0
        }
    }

    /// Kontekst i warstwy na GPU: na CPU (albo bez znanego budżetu VRAM) pełny kontekst i 0
    /// warstw; na GPU — kontekst z [`crate::LocalConfig::ctx_for`] i warstwy mieszczące się
    /// w budżecie po rezerwie na STT (`gpu_layers = N` z konfiguracji ma pierwszeństwo).
    fn ctx_and_layers(
        &self,
        entry: &ModelEntry,
        backend: BackendKey,
        budget: u32,
        rec: Option<&Recommendation>,
    ) -> (u32, u32) {
        let full_ctx = self.config.ctx.min(entry.ctx);
        if backend == BackendKey::Cpu {
            return (full_ctx, 0);
        }
        let reserve = self.stt_reserve(rec);
        let ctx = if budget == 0 {
            full_ctx
        } else {
            self.config.ctx_for(entry, budget, reserve)
        };
        let layers = match self.config.gpu_layers {
            GpuLayers::Fixed(n) => n,
            GpuLayers::Auto => layers_for(entry, budget.saturating_sub(reserve), ctx),
        };
        (ctx, layers)
    }

    /// Urządzenie serwera dla karty z konfiguracji (albo głównej z profilu). `None`, gdy
    /// przypinanie jest wyłączone, serwer widzi najwyżej jedno urządzenie (nie ma czego wybierać)
    /// albo żadne nie pasuje nazwą — model startuje wtedy jak dotąd.
    fn pinned_device(&self, program: &std::path::Path, backend: BackendKey) -> Option<String> {
        if !self.config.pin_device || backend == BackendKey::Cpu {
            return None;
        }
        let name = self.config.gpu_name.clone().or_else(|| {
            let profile = self.device.as_ref()?.current();
            profile.primary_gpu().map(|g| g.name.clone())
        })?;
        // Sonda trwa do kilku sekund — zamek pamięci urządzeń nie jest trzymany na jej czas, a pusty
        // wynik (silnik nie odpowiedział, np. skaner antywirusowy przy zimnym starcie) nie zostaje
        // w pamięci: następne uruchomienie modelu spróbuje ponownie.
        let cached = lock(&self.devices).get(program).cloned();
        let devices = match cached {
            Some(devices) => devices,
            None => {
                let found = probe_blocking(program);
                if !found.is_empty() {
                    lock(&self.devices).insert(program.to_path_buf(), found.clone());
                }
                found
            }
        };
        let explicit = self.config.gpu_name.is_some();
        if devices.len() < 2 && !explicit {
            return None;
        }
        let found = crate::devices::device_for(&devices, backend, &name).map(|d| d.id.clone());
        if found.is_none() {
            tracing::warn!(karta = %name, backend = backend.as_str(), "serwer nie widzi tej karty — start bez przypięcia");
        }
        found
    }

    /// Plan: backend z profilu urządzenia, kontekst, warstwy GPU z dzierżawy/budżetu, wątki.
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
        let gpu = self.target_gpu(backend);
        let budget = self.vram_budget(rec.as_ref(), gpu);
        let (ctx, wanted) = self.ctx_and_layers(entry, backend, budget, rec.as_ref());
        let placement = if wanted == 0 {
            Placement::CpuOnly
        } else {
            Placement::GpuPreferred
        };
        let lease = self.acquire(entry, placement, ctx, wanted, gpu)?;
        let gpu_layers = match &lease {
            // Dzierżawa karty zintegrowanej jest z założenia „w RAM” — warstwy zostają na karcie.
            Some(l) if l.device == Device::Cpu && !self.config.shared_memory => 0,
            _ => wanted,
        };
        if gpu_layers == 0 {
            backend = BackendKey::Cpu;
        }
        let Some(program) = self.config.server(backend) else {
            // Bez pliku silnika model nie wystartuje — dzierżawa pamięci nie może zostać zajęta.
            self.release_lease(lease.map(|l| l.id));
            return Err(LocalError::NoServerBinary(backend.as_str().into()));
        };
        let device = self.pinned_device(&program, backend);
        let plan = LaunchPlan {
            program,
            backend,
            gpu_layers,
            ctx,
            threads,
            device,
        };
        Ok((plan, lease.map(|l| l.id)))
    }
}

/// Sonda urządzeń silnika trwa do kilku sekund i blokuje wątek. W środowisku wielowątkowym
/// oddajemy na ten czas wątek roboczy (`block_in_place`), żeby inne zadania nie stały; poza nim
/// (środowisko jednowątkowe, zwykły wątek) wywołanie jest bezpośrednie.
fn probe_blocking(program: &std::path::Path) -> Vec<crate::devices::EngineDevice> {
    use tokio::runtime::{Handle, RuntimeFlavor};
    let run = || crate::devices::probe(program, crate::devices::PROBE_TIMEOUT);
    match Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(run)
        }
        _ => run(),
    }
}
