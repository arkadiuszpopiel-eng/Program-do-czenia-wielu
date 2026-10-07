//! Laptop właściciela na atrapach (fala 6): i7-13700H (14 rdzeni / 20 wątków), RTX 4050 Laptop
//! 6 GB + Iris Xe, 16 GB RAM — `FakeDeviceProfile::laptop()` (PLAN §3.5 Laptop-CUDA, §6.3 D-CUDA),
//! zarządca rezydencji z budżetem z rekomendacji (6 GB VRAM − rezerwa pulpitu), **prawdziwy wpis
//! manifestu** (Bielik 4.5B Q8_0 albo 1.5B Q8_0) i fałszywy `llama-server` skopiowany do katalogów sidecarów
//! jak w aplikacji (`sidecars/llama-<backend>/`; kolejność kandydatów jak
//! `app_modules::route::local::candidates`).

use std::path::PathBuf;
use std::sync::Arc;

use device_profile_contract::DeviceProfile;
use device_profile_fake::FakeDeviceProfile;
use futures_util::StreamExt;
use model_residency_contract::{
    Budget, LeaseId, LeaseRequest, ModelRole, Placement, Priority, Residency,
};
use model_residency_fake::FakeResidency;
use providers_contract::{CancellationToken, ChatRequest, Message, ModelProvider, ProviderEvent};
use providers_local_impl::{
    BackendKey, LocalConfig, LocalProvider, ModelEntry, RESIDENCY_OWNER, Sidecar, builtin_models,
};

use super::{Env, fake_server};

/// Kolejność kandydatów `llama-server` per backend (jak w kompozycji aplikacji).
pub const ORDER: [(BackendKey, [&str; 4]); 3] = [
    (
        BackendKey::Cuda,
        ["llama-cuda", "llama", "llama-vulkan", "llama-cpu"],
    ),
    (
        BackendKey::Vulkan,
        ["llama-vulkan", "llama", "llama-cpu", "llama-cuda"],
    ),
    (
        BackendKey::Cpu,
        ["llama-cpu", "llama", "llama-vulkan", "llama-cuda"],
    ),
];

/// Budżet VRAM zarządcy na laptopie: 5921 MB (DXGI) − 768 MB rezerwy pulpitu.
pub const VRAM_BUDGET: u32 = 5_921 - 768;

pub fn server_path(env: &Env, dir: &str) -> PathBuf {
    let name = fake_server().file_name().unwrap().to_owned();
    env.dir.path().join("sidecars").join(dir).join(name)
}

pub fn install_server(env: &Env, dir: &str) -> PathBuf {
    let path = server_path(env, dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::copy(fake_server(), &path).unwrap();
    path
}

/// Wpis manifestu po identyfikatorze (`bielik-4.5b-…` domyślny, `bielik-1.5b-…` lekki).
pub fn bielik(prefix: &str) -> ModelEntry {
    let models = builtin_models().unwrap();
    let entry = models.into_iter().find(|m| m.id.starts_with(prefix));
    entry.unwrap_or_else(|| panic!("brak modelu {prefix} w manifeście"))
}

pub struct Laptop {
    pub env: Env,
    pub device: Arc<FakeDeviceProfile>,
    pub residency: Arc<FakeResidency>,
    pub provider: LocalProvider,
    pub model: ModelEntry,
}

/// Laptop z domyślnym modelem (Bielik 4.5B Q8_0).
pub fn laptop(installed: &[&str]) -> Laptop {
    laptop_with(installed, "bielik-4.5b", |_| {})
}

/// Laptop z zainstalowanymi kompilacjami `llama-server`, modelem i poprawką konfiguracji.
pub fn laptop_with(
    installed: &[&str],
    model: &str,
    tweak: impl FnOnce(&mut LocalConfig),
) -> Laptop {
    let env = Env::new();
    let device = Arc::new(FakeDeviceProfile::laptop());
    let budget = Budget::from_device(&device.recommend().residency);
    assert_eq!(budget.vram_mb, VRAM_BUDGET);
    let residency = Arc::new(FakeResidency::new(budget));
    let model = bielik(model);
    super::install(&env.models(), &model);
    for dir in installed {
        install_server(&env, dir);
    }
    let mut config: LocalConfig = env.config();
    config.default_model = model.id.clone();
    for (key, dirs) in ORDER {
        config.server_bin.insert(key, server_path(&env, "llama"));
        let candidates = dirs.iter().map(|d| server_path(&env, d)).collect();
        config.server_candidates.insert(key, candidates);
    }
    tweak(&mut config);
    let sidecar = Sidecar::new(
        config,
        Arc::new(env.launcher("ok", None)),
        Some(device.clone() as Arc<dyn DeviceProfile>),
        Some(residency.clone() as Arc<dyn Residency>),
    )
    .unwrap();
    let provider = LocalProvider::new(vec![model.clone()], sidecar);
    Laptop {
        env,
        device,
        residency,
        provider,
        model,
    }
}

impl Laptop {
    /// Jedna tura rozmowy z modelem lokalnym (tekst odpowiedzi).
    pub async fn ask(&self, text: &str) -> String {
        let req = ChatRequest::new(self.model.id.clone(), vec![Message::user_text(text)]);
        let events: Vec<_> = self
            .provider
            .stream(req, CancellationToken::new())
            .collect()
            .await;
        events
            .iter()
            .filter_map(|e| match e {
                ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Dzierżawa LLM (`providers-local`), jeśli jest.
    pub fn llm_lease(&self) -> Option<model_residency_contract::Lease> {
        let leases = self.residency.snapshot().leases;
        leases
            .into_iter()
            .find(|l| l.request.owner == RESIDENCY_OWNER)
    }

    /// Dzierżawa STT jak w `voice-stt-impl` (`engine_start.rs`) i przypięcie jej przez potok głosu
    /// na czas rozmowy (`voice-pipeline-impl` `ResidencyPin`).
    pub fn start_stt(&self) -> model_residency_contract::Grant {
        let grant = self.residency.acquire(stt_lease()).unwrap();
        self.residency.set_in_use(grant.lease.id, true).unwrap();
        grant
    }

    /// Upływ czasu zarządcy i jego zadanie tła (zwalnianie bezczynnych).
    pub fn tick(&self, ms: u64) -> Vec<LeaseId> {
        self.residency.clock().advance_ms(ms);
        self.residency
            .reap_idle()
            .into_iter()
            .map(|r| r.lease.id)
            .collect()
    }
}

/// Żądanie dzierżawy STT `whisper-server` CUDA (`voice-stt-impl::WhisperStt::lease_request`).
pub fn stt_lease() -> LeaseRequest {
    LeaseRequest {
        owner: "voice-stt".into(),
        model: "ggml-large-v3-turbo-q5_0.bin".into(),
        role: ModelRole::Stt,
        priority: Priority::VoiceRt,
        placement: Placement::GpuIfFree,
        vram_mb: 1_500,
        ram_mb: 600,
        cpu_ram_mb: 1_500,
        idle_unload_ms: 600_000,
    }
}

/// Ciężki TTS na GPU (Chatterbox/XTTS — profil D-CUDA, gdy zmieści się na przemian z STT).
pub fn heavy_tts_lease() -> LeaseRequest {
    LeaseRequest {
        owner: "voice-tts".into(),
        model: "heavy-tts".into(),
        role: ModelRole::Tts,
        priority: Priority::VoiceRt,
        placement: Placement::GpuPreferred,
        vram_mb: 1_200,
        ram_mb: 600,
        cpu_ram_mb: 1_500,
        idle_unload_ms: 0,
    }
}
