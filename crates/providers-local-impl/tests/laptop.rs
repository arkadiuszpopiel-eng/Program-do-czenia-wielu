//! Próba profilu laptopa właściciela (fala 6, na atrapach): i7-13700H (14 rdzeni / 20 wątków),
//! RTX 4050 Laptop 6 GB + Iris Xe, 16 GB RAM — `FakeDeviceProfile::laptop()` (PLAN §3.5
//! Laptop-CUDA, §6.3 D-CUDA), zarządca rezydencji z budżetem z rekomendacji (6 GB VRAM − rezerwa
//! pulpitu), **prawdziwy wpis manifestu** (Bielik 4.5B Q4_K_M) i fałszywy `llama-server`
//! skopiowany do katalogów sidecarów jak w aplikacji (`sidecars/llama-<backend>/`; kolejność
//! kandydatów jak `app_modules::route::local::candidates`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use device_profile_contract::{
    Backend, DeviceProfile, HwClass, LocalLlm, PowerState, SttModel, VoiceProfile, VoiceVariant,
};
use device_profile_fake::FakeDeviceProfile;
use futures_util::StreamExt;
use model_residency_contract::{
    Budget, Device, LeaseRequest, ModelRole, Placement, Priority, Residency,
};
use model_residency_fake::FakeResidency;
use providers_contract::{CancellationToken, ChatRequest, Message, ModelProvider, ProviderEvent};
use providers_local_impl::{
    BackendKey, LocalConfig, LocalProvider, ModelEntry, Sidecar, builtin_models,
};
use support::{Env, arg, fake_server};

const ORDER: [(BackendKey, [&str; 4]); 3] = [
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

fn server_path(env: &Env, dir: &str) -> PathBuf {
    let name = fake_server().file_name().unwrap().to_owned();
    env.dir.path().join("sidecars").join(dir).join(name)
}

fn install_server(env: &Env, dir: &str) -> PathBuf {
    let path = server_path(env, dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::copy(fake_server(), &path).unwrap();
    path
}

fn bielik() -> ModelEntry {
    let models = builtin_models().unwrap();
    let entry = models.into_iter().next().unwrap();
    assert!(entry.id.starts_with("bielik-4.5b"), "{}", entry.id);
    entry
}

struct Laptop {
    env: Env,
    device: Arc<FakeDeviceProfile>,
    residency: Arc<FakeResidency>,
    provider: LocalProvider,
    model: ModelEntry,
}

fn laptop(installed: &[&str]) -> Laptop {
    let env = Env::new();
    let device = Arc::new(FakeDeviceProfile::laptop());
    let budget = Budget::from_device(&device.recommend().residency);
    let residency = Arc::new(FakeResidency::new(budget));
    let model = bielik();
    support::install(&env.models(), &model);
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

async fn ask(l: &Laptop, text: &str) -> String {
    let req = ChatRequest::new(l.model.id.clone(), vec![Message::user_text(text)]);
    let events: Vec<_> = l
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

fn voice_lease(role: ModelRole, vram_mb: u32) -> LeaseRequest {
    LeaseRequest {
        owner: format!("voice-{role:?}").to_lowercase(),
        model: format!("{role:?}"),
        role,
        priority: Priority::VoiceRt,
        placement: Placement::GpuPreferred,
        vram_mb,
        ram_mb: 600,
        cpu_ram_mb: 1_500,
        idle_unload_ms: 0,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_ac_power_bielik_runs_on_cuda_build_and_stt_fits_in_6_gb() {
    let l = laptop(&["llama-cuda", "llama-vulkan", "llama-cpu"]);
    let rec = l.device.recommend();
    assert_eq!(rec.class, HwClass::LaptopCuda);
    assert_eq!(
        (rec.voice_profile, rec.voice_variant),
        (VoiceProfile::D, Some(VoiceVariant::Cuda))
    );
    assert_eq!(
        (rec.stt_backend, rec.stt_model),
        (Backend::Cuda, SttModel::LargeV3TurboQ5)
    );
    assert_eq!(
        (rec.llm_backend, rec.local_llm),
        (Backend::Cuda, LocalLlm::Small)
    );
    assert!(
        rec.residency.stt_tts_exclusive,
        "STT i ciężki TTS nie naraz"
    );
    assert_eq!(rec.residency.vram_mb, 5_921 - 768);
    assert!(l.model.quant == "Q4_K_M" && l.model.params_b <= 4.6);
    assert!(
        l.model.vram_mb <= rec.residency.vram_mb,
        "Bielik mieści się w VRAM"
    );

    assert_eq!(ask(&l, "cześć").await, "Echo: cześć");
    let (_, plan, _) = l.provider.sidecar().running_plan().await.unwrap();
    assert_eq!(plan.program, server_path(&l.env, "llama-cuda"));
    assert_eq!(plan.backend, BackendKey::Cuda);
    let args = l.env.launches().remove(0);
    assert_eq!(
        arg(&args, "-ngl"),
        Some(l.model.layers.to_string()),
        "pełne odciążenie"
    );
    assert_eq!(
        arg(&args, "--threads").as_deref(),
        Some("14"),
        "rdzenie fizyczne"
    );
    assert_eq!(arg(&args, "-c").as_deref(), Some("8192"));
    assert!(args.contains(&"--jinja".to_owned()));
    let llm = l.residency.snapshot().leases[0].clone();
    assert_eq!((llm.device, llm.request.vram_mb), (Device::Gpu, 3_400));

    // STT whisper turbo na CUDA obok LLM: 3400 + 1500 ≤ 5153 MB — bez eksmisji.
    let stt = l
        .residency
        .acquire(voice_lease(ModelRole::Stt, 1_500))
        .unwrap();
    assert_eq!(stt.lease.device, Device::Gpu);
    assert!(stt.evicted.is_empty(), "{:?}", stt.evicted);
    let state = l.residency.snapshot();
    assert!(state.used.vram_mb <= state.budget.vram_mb);
    // Ciężki TTS na GPU wyklucza STT (laptop 6 GB), LLM zostaje.
    let tts = l
        .residency
        .acquire(voice_lease(ModelRole::Tts, 1_200))
        .unwrap();
    let evicted: Vec<_> = tts.evicted.iter().map(|r| r.lease.id).collect();
    assert_eq!(
        evicted,
        [stt.lease.id],
        "STT i ciężki TTS nie są rezydentne naraz"
    );
    assert!(
        l.provider.sidecar().running_plan().await.is_some(),
        "LLM nadal załadowany"
    );
    l.provider.sidecar().stop("test").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_battery_local_llm_moves_to_cpu_build_without_gpu_layers() {
    let l = laptop(&["llama-cuda", "llama-cpu"]);
    l.device
        .set_power(PowerState::Battery { percent: Some(30) });
    let rec = l.device.recommend();
    assert!(rec.power_saving);
    assert_eq!(
        (rec.voice_profile, rec.local_llm),
        (VoiceProfile::B, LocalLlm::None)
    );
    assert_eq!(ask(&l, "bateria").await, "Echo: bateria");
    let (_, plan, _) = l.provider.sidecar().running_plan().await.unwrap();
    assert_eq!(plan.program, server_path(&l.env, "llama-cpu"));
    assert_eq!((plan.backend, plan.gpu_layers), (BackendKey::Cpu, 0));
    let lease = l.residency.snapshot().leases[0].clone();
    assert_eq!(lease.device, Device::Cpu);
    l.provider.sidecar().stop("test").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_cuda_build_the_vulkan_build_keeps_the_model_on_gpu() {
    let l = laptop(&["llama-vulkan", "llama-cpu"]);
    assert_eq!(ask(&l, "vulkan").await, "Echo: vulkan");
    let (_, plan, _) = l.provider.sidecar().running_plan().await.unwrap();
    assert_eq!(
        plan.program,
        server_path(&l.env, "llama-vulkan"),
        "zastępstwo CUDA → Vulkan"
    );
    assert_eq!(plan.gpu_layers, l.model.layers);
    // Pobranie wersji CUDA w trakcie pracy: następny start sidecara używa jej bez restartu Alfy.
    install_server(&l.env, "llama-cuda");
    l.provider.sidecar().stop("zmiana silnika").await;
    assert_eq!(ask(&l, "cuda").await, "Echo: cuda");
    let (_, plan, _) = l.provider.sidecar().running_plan().await.unwrap();
    assert_eq!(plan.program, server_path(&l.env, "llama-cuda"));
    l.provider.sidecar().stop("test").await;
}
