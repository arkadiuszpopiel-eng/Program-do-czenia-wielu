//! Start sidecara `whisper-server` dla [`WhisperStt`]: dzierżawa `model-residency`, uruchomienie
//! procesu i czekanie na `/health`. Nieudany start kompilacji GPU (np. wersja CUDA bez sterownika
//! NVIDIA albo bez bibliotek `cudart`) → start na CPU w tej samej wypowiedzi (zdarzenie
//! `BackendFallback`, stan `Degraded`), zamiast błędu rozpoznawania aż do restartu aplikacji.

use std::time::{Duration, Instant};

use device_profile_contract::Backend;
use model_residency_contract::{Device, LeaseRequest, ModelRole, Placement, Priority};
use voice_stt_contract::{Health, SttError, SttEvent};

use super::{OWNER, Server, WhisperStt};
use crate::sidecar::{LaunchSpec, build_args, free_port};

impl WhisperStt {
    /// Dzierżawa modelu. Na GPU tylko z wolnego miejsca (`GpuIfFree`): STT ma dobrą wersję CPU,
    /// a wyparcie lokalnego LLM z karty (laptop 6 GB) oznaczałoby jego przeładowanie na CPU
    /// w tej samej rozmowie — STT idzie wtedy na CPU, LLM zostaje na karcie.
    pub(super) fn lease_request(&self, model: &str, backend: Backend) -> LeaseRequest {
        LeaseRequest {
            owner: OWNER.into(),
            model: model.into(),
            role: ModelRole::Stt,
            priority: Priority::VoiceRt,
            placement: if backend == Backend::Cpu {
                Placement::CpuOnly
            } else {
                Placement::GpuIfFree
            },
            vram_mb: 1_500,
            ram_mb: 600,
            cpu_ram_mb: 1_500,
            idle_unload_ms: self.config.idle_unload_ms,
        }
    }

    pub(super) fn release(&self, server: Server) {
        server.sidecar.kill();
        self.release_lease(server.lease);
    }

    fn release_lease(&self, lease: Option<model_residency_contract::LeaseId>) {
        if let (Some(r), Some(id)) = (&self.residency, lease) {
            let _ = r.release(id);
        }
    }

    /// Serwer dla backendu; nieudany start GPU → CPU (backend oznaczony jako niesprawny).
    pub(super) async fn start_with_fallback(
        &self,
        model: &str,
        backend: Backend,
    ) -> Result<Server, SttError> {
        match self.start_server(model, backend).await {
            Err(e) if backend != Backend::Cpu => {
                {
                    let mut st = self.lock();
                    st.failed.push(backend);
                    st.events.push(SttEvent::BackendFallback {
                        from: backend,
                        to: Backend::Cpu,
                        reason: format!("start nieudany: {e}"),
                    });
                }
                self.start_server(model, Backend::Cpu).await
            }
            result => result,
        }
    }

    async fn start_server(&self, model: &str, mut backend: Backend) -> Result<Server, SttError> {
        let mut lease = None;
        if let Some(r) = &self.residency {
            let grant = r
                .acquire(self.lease_request(model, backend))
                .map_err(|e| SttError::Sidecar(format!("rezydencja: {e}")))?;
            if grant.lease.device == Device::Cpu {
                backend = Backend::Cpu;
            }
            lease = Some(grant.lease.id);
        }
        self.lock().health = Some(Health::Starting);
        let failed = |msg: String| {
            self.release_lease(lease);
            self.lock().health = Some(Health::Failed(msg.clone()));
            SttError::Sidecar(msg)
        };
        let port = free_port().map_err(|e| failed(e.to_string()))?;
        let program = self
            .config
            .binaries
            .for_backend(backend)
            .cloned()
            .unwrap_or_else(|| self.config.binaries.cpu.clone());
        let spec = LaunchSpec {
            program,
            args: build_args(&self.config, backend, port),
            port,
            backend,
        };
        let sidecar = self
            .launcher
            .launch(&spec)
            .await
            .map_err(|e| failed(e.to_string()))?;
        let base = sidecar.base_url();
        let deadline = Instant::now() + self.config.startup_timeout;
        while !self.client.healthy(&base).await {
            if sidecar.exited().is_some() || Instant::now() > deadline {
                sidecar.kill();
                return Err(failed(format!(
                    "whisper-server ({backend:?}) nie wystartował"
                )));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(Server {
            sidecar,
            backend,
            lease,
        })
    }
}
