//! Próba profilu laptopa właściciela (fala 6, na atrapach — `support::laptop`): RTX 4050 Laptop
//! 6 GB (budżet zarządcy 5153 MB), 16 GB RAM, Bielik z manifestu. Dzierżawa LLM liczy KV cache
//! i tyle warstw, ile trafia na kartę; obok LLM zostaje miejsce na whisper CUDA (bez OOM).
//! 4.5B Q8_0 (5,4 GB bez KV) nie mieści się w całości → 34 z 60 warstw na karcie (3570 MB),
//! 1.5B Q8_0 — w całości (2456 MB).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use device_profile_contract::{
    Backend, DeviceProfile, HwClass, LocalLlm, PowerState, SttModel, VoiceProfile, VoiceVariant,
};
use model_residency_contract::{Device, Residency};
use providers_local_impl::{BackendKey, STT_VRAM_RESERVE_MB};
use support::arg;
use support::laptop::{
    VRAM_BUDGET, heavy_tts_lease, install_server, laptop, laptop_with, server_path, stt_lease,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_ac_power_bielik_q8_is_split_on_cuda_and_stt_fits_in_6_gb() {
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
    assert_eq!(rec.residency.vram_mb, VRAM_BUDGET);
    assert!(l.model.quant == "Q8_0" && l.model.params_b <= 4.8);
    // Q8_0 z KV nie mieści się w całości obok STT nawet przy -c 4096 (5640 + 1500 > 5153 MB).
    assert!(l.model.vram_need(4_096) + STT_VRAM_RESERVE_MB > VRAM_BUDGET);

    assert_eq!(l.ask("cześć").await, "Echo: cześć");
    let (_, plan, _) = l.provider.sidecar().running_plan().await.unwrap();
    assert_eq!(plan.program, server_path(&l.env, "llama-cuda"));
    assert_eq!(
        (plan.backend, plan.ctx, plan.gpu_layers),
        (BackendKey::Cuda, 8_192, 34)
    );
    let args = l.env.launches().remove(0);
    assert_eq!(
        arg(&args, "-ngl").as_deref(),
        Some("34"),
        "częściowe odciążenie"
    );
    assert_eq!(
        arg(&args, "--threads").as_deref(),
        Some("14"),
        "rdzenie fizyczne"
    );
    assert_eq!(arg(&args, "-c").as_deref(), Some("8192"));
    assert!(args.contains(&"--jinja".to_owned()));
    let body: serde_json::Value = serde_json::from_str(&l.env.requests()[0]).unwrap();
    assert_eq!(body["max_tokens"], 4_096, "połowa kontekstu uruchomienia");
    let llm = l.llm_lease().unwrap();
    assert_eq!((llm.device, llm.request.vram_mb), (Device::Gpu, 3_570));
    assert_eq!(
        llm.request.ram_mb, 3_146,
        "26 warstw na CPU: wagi i KV w RAM"
    );
    assert_eq!(llm.request.cpu_ram_mb, 5_600 + 480);

    // STT whisper turbo na CUDA obok LLM: 3570 + 1500 = 5070 ≤ 5153 MB — bez eksmisji (poza
    // rozmową, bez przypięcia przez potok głosu).
    let stt = l.residency.acquire(stt_lease()).unwrap();
    assert_eq!(stt.lease.device, Device::Gpu);
    assert!(stt.evicted.is_empty(), "{:?}", stt.evicted);
    let state = l.residency.snapshot();
    assert_eq!(state.used.vram_mb, 3_570 + 1_500);
    assert!(state.used.vram_mb <= state.budget.vram_mb);
    assert!(state.used.ram_mb <= state.budget.ram_mb);
    // Ciężki TTS na GPU wyklucza STT (laptop 6 GB), LLM zostaje.
    let tts = l.residency.acquire(heavy_tts_lease()).unwrap();
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
async fn light_bielik_fits_entirely_next_to_stt() {
    let l = laptop_with(&["llama-cuda", "llama-cpu"], "bielik-1.5b", |_| {});
    assert_eq!(l.ask("lekki").await, "Echo: lekki");
    let (_, plan, _) = l.provider.sidecar().running_plan().await.unwrap();
    assert_eq!((plan.ctx, plan.gpu_layers), (8_192, l.model.layers));
    let args = l.env.launches().remove(0);
    assert!(!args.contains(&"--jinja".to_owned()), "bez narzędzi");
    let llm = l.llm_lease().unwrap();
    assert_eq!(
        (llm.request.vram_mb, llm.request.ram_mb),
        (2_200 + 256, 512)
    );
    let stt = l.residency.acquire(stt_lease()).unwrap();
    assert_eq!(stt.lease.device, Device::Gpu);
    assert_eq!(l.residency.snapshot().used.vram_mb, 2_456 + 1_500);
    l.provider.sidecar().stop("test").await;
}

/// Większa rezerwa na STT (np. cięższy model rozpoznawania): 1.5B mieści się w całości tylko
/// z kontekstem 4096 — kontekst maleje, `max_tokens` liczony z kontekstu uruchomienia.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn context_shrinks_to_keep_the_whole_model_on_gpu() {
    let l = laptop_with(&["llama-cuda"], "bielik-1.5b", |c| c.stt_reserve_mb = 2_800);
    assert_eq!(l.ask("mniej kontekstu").await, "Echo: mniej kontekstu");
    let (_, plan, _) = l.provider.sidecar().running_plan().await.unwrap();
    assert_eq!((plan.ctx, plan.gpu_layers), (4_096, l.model.layers));
    assert_eq!(arg(&l.env.launches()[0], "-c").as_deref(), Some("4096"));
    let body: serde_json::Value = serde_json::from_str(&l.env.requests()[0]).unwrap();
    assert_eq!(
        body["max_tokens"], 2_048,
        "połowa -c 4096, nie konfiguracji"
    );
    assert_eq!(l.llm_lease().unwrap().request.vram_mb, 2_200 + 128);
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
    assert_eq!(l.ask("bateria").await, "Echo: bateria");
    let (_, plan, _) = l.provider.sidecar().running_plan().await.unwrap();
    assert_eq!(plan.program, server_path(&l.env, "llama-cpu"));
    assert_eq!((plan.backend, plan.gpu_layers), (BackendKey::Cpu, 0));
    assert_eq!(plan.ctx, 8_192, "na CPU pełny kontekst (KV w RAM)");
    let lease = l.llm_lease().unwrap();
    assert_eq!(lease.device, Device::Cpu);
    assert_eq!(lease.request.cpu_ram_mb, 5_600 + 480, "KV cache w RAM");
    l.provider.sidecar().stop("test").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_cuda_build_the_vulkan_build_keeps_the_model_on_gpu() {
    let l = laptop(&["llama-vulkan", "llama-cpu"]);
    assert_eq!(l.ask("vulkan").await, "Echo: vulkan");
    let (_, plan, _) = l.provider.sidecar().running_plan().await.unwrap();
    assert_eq!(
        plan.program,
        server_path(&l.env, "llama-vulkan"),
        "zastępstwo CUDA → Vulkan"
    );
    assert_eq!(plan.gpu_layers, 34);
    // Pobranie wersji CUDA w trakcie pracy: następny start sidecara używa jej bez restartu Alfy.
    install_server(&l.env, "llama-cuda");
    l.provider.sidecar().stop("zmiana silnika").await;
    assert_eq!(l.ask("cuda").await, "Echo: cuda");
    let (_, plan, _) = l.provider.sidecar().running_plan().await.unwrap();
    assert_eq!(plan.program, server_path(&l.env, "llama-cuda"));
    l.provider.sidecar().stop("test").await;
}
