//! Konfiguracja i plan uruchomienia: backend z profilu, warstwy GPU wg VRAM, redakcja, błędy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use device_profile_contract::DeviceProfile;
use device_profile_contract::{Backend, PowerState, recommend};
use device_profile_fake::FakeDeviceProfile;
use providers_contract::ProviderErrorKind;
use providers_local_impl::{
    BackendChoice, BackendKey, GPU_OVERHEAD_MB, LaunchPlan, LaunchSpec, LocalConfig, LocalError,
    LocalEvent, MIN_CTX, ModelEntry, PROCESS_RAM_MB, STT_VRAM_RESERVE_MB, builtin_models,
    layers_for,
};

#[test]
fn backend_follows_device_profile_and_battery() {
    let config = LocalConfig::new("/m", "/bin/llama-server");
    let desktop = FakeDeviceProfile::desktop().recommend();
    assert_eq!(config.backend_for(&desktop), BackendKey::Vulkan);
    let laptop = FakeDeviceProfile::laptop();
    assert_eq!(config.backend_for(&laptop.recommend()), BackendKey::Cuda);
    laptop.set_power(PowerState::Battery { percent: Some(30) });
    assert_eq!(
        config.backend_for(&laptop.recommend()),
        BackendKey::Cpu,
        "bateria: CPU"
    );
    let forced = LocalConfig {
        backend: BackendChoice::Fixed(Backend::Cpu),
        ..config.clone()
    };
    assert_eq!(forced.backend_for(&desktop), BackendKey::Cpu);
    let baseline = recommend(&device_profile_contract::fixtures::baseline());
    assert_eq!(config.backend_for(&baseline), BackendKey::Vulkan);
}

#[test]
fn gpu_layers_and_memory_count_kv_cache_and_overhead() {
    let e = support::entry("https://x/m.gguf");
    // 3000 MB wag i narzutu + 64 MB KV na 1024 tokeny: -c 8192 → 3512 MB.
    assert_eq!(e.vram_need(8_192), 3_512);
    assert_eq!(layers_for(&e, 3_512, 8_192), 32);
    assert_eq!(layers_for(&e, 8_000, 8_192), 32);
    assert_eq!(layers_for(&e, 3_000, 0), 32, "bez KV mieści się w całości");
    // Częściowo: (budżet − 550 MB narzutu) / (3512 − 550) MB na 32 warstwy.
    assert_eq!(
        layers_for(&e, 3_000, 8_192),
        26,
        "bez miejsca na KV — mniej warstw"
    );
    assert_eq!(layers_for(&e, 1_756, 8_192), 13);
    assert_eq!(layers_for(&e, GPU_OVERHEAD_MB, 8_192), 0);
    assert_eq!(layers_for(&e, 0, 8_192), 0);
    assert_eq!(e.vram_for(26, 8_192), GPU_OVERHEAD_MB + 2_962 * 26 / 32);
    assert!(e.vram_for(26, 8_192) <= 3_000);
    assert_eq!((e.vram_for(0, 8_192), e.vram_for(32, 8_192)), (0, 3_512));
    assert_eq!(e.vram_for(99, 8_192), 3_512);
    assert_eq!(
        e.ram_for(32, 8_192),
        PROCESS_RAM_MB,
        "pełne odciążenie: sam proces"
    );
    assert_eq!(e.ram_for(16, 8_192), PROCESS_RAM_MB + (3_500 + 512) / 2);
    assert_eq!(e.ram_for(0, 8_192), PROCESS_RAM_MB + e.ram_need(8_192));
}

#[test]
fn bielik_q8_kv_cache_and_layout_on_the_6_gb_laptop() {
    let models = builtin_models().unwrap();
    let (b45, b15) = (&models[0], &models[1]);
    assert_eq!(b45.id, "bielik-4.5b-v3.0-instruct-q8_0");
    assert_eq!(b15.id, "bielik-1.5b-v3.0-instruct-q8_0");
    assert_eq!(LocalConfig::new("/m", "/s").default_model, b45.id);
    // 60 warstw × 2 głowice KV × 128 × 2 (K, V) × 2 B = 60 MiB na 1024 tokeny.
    assert_eq!(b45.kv_mb_per_1k_ctx, 60);
    assert_eq!((b45.kv_mb(8_192), b45.kv_mb(4_096)), (480, 240));
    assert_eq!(b45.vram_need(8_192), 5_400 + 480);
    assert_eq!(b45.ram_need(8_192), 5_600 + 480);
    assert_eq!(b45.kv_mb(1), 1, "w górę");
    assert_eq!(
        (b15.kv_mb_per_1k_ctx, b15.layers, b15.tools),
        (32, 32, false)
    );
    let config = LocalConfig::new("/m", "/bin/llama-server");
    assert_eq!((config.ctx, config.min_ctx), (8_192, MIN_CTX));
    assert_eq!(config.stt_reserve_mb, STT_VRAM_RESERVE_MB);
    let laptop = 5_921 - 768;
    // 4.5B Q8_0 nie mieści się w całości nawet przy -c 4096 → pełny kontekst, część warstw:
    // budżet − rezerwa STT = 3653 MB → 34 z 60 warstw (3570 MB), whisper CUDA obok (5070 MB).
    assert_eq!(config.ctx_for(b45, laptop, STT_VRAM_RESERVE_MB), 8_192);
    let layers = layers_for(b45, laptop - STT_VRAM_RESERVE_MB, 8_192);
    assert_eq!(layers, 34);
    assert_eq!(b45.vram_for(layers, 8_192), 3_570);
    assert_eq!(b45.ram_for(layers, 8_192), 3_146);
    assert!(b45.vram_for(layers, 8_192) + STT_VRAM_RESERVE_MB <= laptop);
    // Bez rezerwy (STT na CPU) — 51 warstw (5080 MB).
    assert_eq!(layers_for(b45, laptop, 8_192), 51);
    // 1.5B Q8_0: w całości obok STT, pełny kontekst (2456 + 1500 MB).
    assert_eq!(config.ctx_for(b15, laptop, STT_VRAM_RESERVE_MB), 8_192);
    assert_eq!(layers_for(b15, laptop - STT_VRAM_RESERVE_MB, 8_192), 32);
    // Baseline 8 GB: 4.5B Q8_0 w całości obok STT (5880 + 1500 ≤ 7408), desktop 16 GB tak samo.
    assert_eq!(config.ctx_for(b45, 8_176 - 768, STT_VRAM_RESERVE_MB), 8_192);
    assert_eq!(
        layers_for(b45, 8_176 - 768 - STT_VRAM_RESERVE_MB, 8_192),
        60
    );
    assert_eq!(
        config.ctx_for(b45, 16_304 - 768, STT_VRAM_RESERVE_MB),
        8_192
    );
}

#[test]
fn context_shrinks_only_when_it_allows_full_offload() {
    // Model ~4,5B Q4_K_M (3400 MB bez KV) na laptopie: 3880 + 1500 > 5153 → -c 4096 (5140 MB).
    let q4 = ModelEntry {
        vram_mb: 3_400,
        ..builtin_models().unwrap().remove(0)
    };
    let config = LocalConfig::new("/m", "/bin/llama-server");
    let laptop = 5_921 - 768;
    assert_eq!(config.ctx_for(&q4, laptop, STT_VRAM_RESERVE_MB), 4_096);
    assert_eq!(config.ctx_for(&q4, laptop, 0), 8_192, "STT na CPU");
    assert_eq!(
        config.ctx_for(&q4, 2_000, STT_VRAM_RESERVE_MB),
        8_192,
        "i tak częściowo"
    );
    let fixed = LocalConfig {
        min_ctx: 8_192,
        ..config.clone()
    };
    assert_eq!(fixed.ctx_for(&q4, laptop, STT_VRAM_RESERVE_MB), 8_192);
    let big = LocalConfig {
        ctx: 32_768,
        ..config
    };
    assert_eq!(big.ctx_for(&q4, laptop, STT_VRAM_RESERVE_MB), 4_096);
    assert_eq!(big.ctx_for(&q4, 16_304 - 768, STT_VRAM_RESERVE_MB), 32_768);
}

#[test]
fn launch_args_are_localhost_only_and_secret_is_redacted() {
    let e = support::entry("https://x/m.gguf");
    let plan = LaunchPlan {
        program: "/bin/llama-server".into(),
        backend: BackendKey::Vulkan,
        gpu_layers: 32,
        ctx: 4_096,
        threads: 6,
    };
    let args = plan.args(&e, std::path::Path::new("/m/x.gguf"), 4321, "tajny-klucz");
    assert_eq!(support::arg(&args, "--host").as_deref(), Some("127.0.0.1"));
    assert!(!args.iter().any(|a| a == "0.0.0.0"));
    assert_eq!(support::arg(&args, "--port").as_deref(), Some("4321"));
    assert_eq!(support::arg(&args, "--threads").as_deref(), Some("6"));
    let spec = LaunchSpec {
        program: plan.program.clone(),
        args,
        secret: "tajny-klucz".into(),
    };
    assert!(!format!("{spec:?}").contains("tajny-klucz"));
    assert_eq!(BackendKey::of(Backend::Cuda).as_str(), "cuda");
}

#[test]
fn errors_map_to_provider_errors_and_events_have_names() {
    assert_eq!(
        LocalError::UnknownModel("x".into())
            .to_provider_error()
            .kind,
        ProviderErrorKind::InvalidRequest
    );
    assert_eq!(
        LocalError::Startup("x".into()).to_provider_error().kind,
        ProviderErrorKind::Server { status: 503 }
    );
    assert_eq!(
        LocalError::NotInstalled("x".into())
            .to_provider_error()
            .kind,
        ProviderErrorKind::Unsupported
    );
    let io: LocalError = std::io::Error::other("dysk").into();
    assert!(io.to_string().contains("dysk"));
    let ev = LocalEvent::BackendFallback {
        from: "vulkan".into(),
        to: "cpu".into(),
        reason: "x".into(),
    };
    assert_eq!(ev.name(), "local.backend.fallback");
    assert_eq!(
        serde_json::to_value(&ev).unwrap()["event"],
        "backend_fallback"
    );
    for (ev, name) in [
        (
            LocalEvent::SidecarCrashed {
                model: "m".into(),
                exit_code: Some(9),
            },
            "local.sidecar.crashed",
        ),
        (
            LocalEvent::DownloadFailed {
                model: "m".into(),
                error: "e".into(),
            },
            "local.model.download.failed",
        ),
    ] {
        assert_eq!(ev.name(), name);
    }
}
