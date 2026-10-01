//! Konfiguracja i plan uruchomienia: backend z profilu, warstwy GPU wg VRAM, redakcja, błędy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use device_profile_contract::DeviceProfile;
use device_profile_contract::{Backend, PowerState, recommend};
use device_profile_fake::FakeDeviceProfile;
use providers_contract::ProviderErrorKind;
use providers_local_impl::{
    BackendChoice, BackendKey, LaunchPlan, LaunchSpec, LocalConfig, LocalError, LocalEvent,
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
fn gpu_layers_by_vram_budget() {
    let e = support::entry("https://x/m.gguf");
    assert_eq!(layers_for(&e, 3_000), 32);
    assert_eq!(layers_for(&e, 8_000), 32);
    assert_eq!(layers_for(&e, 1_500), 16);
    assert_eq!(layers_for(&e, 0), 0);
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
