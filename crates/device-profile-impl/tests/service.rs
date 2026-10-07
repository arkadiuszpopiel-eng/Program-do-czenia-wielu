//! Testy usługi na atrapie `HardwarePort` (dane maszyn z planu) i na magistrali-atrapie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use device_profile_contract::{
    DeviceEvent, DeviceProfile, EVENT_DETECTED, EVENT_POWER_CHANGED, HwClass, MachineOverlay,
    PowerState, VoiceChoice, VoiceProfile, contract_tests, event_kind,
};
use device_profile_impl::{
    DeviceProfileConfig, DeviceProfileService, MODULE_TOML, derive_machine_id,
};
use platform_contract::{
    AudioDirection, AudioEndpoint, CpuSummary, GpuAdapter, HardwarePort, OsSummary, PlatformError,
    PowerStatus, WindowId, WindowInfo, WindowPort,
};

/// Laptop z planu (i7-13700H, RTX 4050 6 GB, 16 GB, bateria) jako surowe dane sondy.
struct LaptopProbe {
    power: Mutex<PowerStatus>,
    vram_mb: Mutex<u64>,
}

impl LaptopProbe {
    fn new() -> Self {
        Self {
            power: Mutex::new(PowerStatus {
                ac_online: Some(true),
                battery_present: true,
                battery_percent: Some(90),
            }),
            vram_mb: Mutex::new(5_921),
        }
    }
}

impl HardwarePort for LaptopProbe {
    fn os(&self) -> Result<OsSummary, PlatformError> {
        Ok(OsSummary {
            name: "Windows 11".into(),
            version: "24H2".into(),
            build: Some(26_100),
        })
    }
    fn cpu(&self) -> Result<CpuSummary, PlatformError> {
        Ok(CpuSummary {
            model: "13th Gen Intel(R) Core(TM) i7-13700H".into(),
            physical_cores: 14,
            logical_cores: 20,
            l3_cache_kb: Some(24 * 1024),
        })
    }
    fn memory_total_mb(&self) -> Result<u64, PlatformError> {
        Ok(16_384)
    }
    fn gpus(&self) -> Result<Vec<GpuAdapter>, PlatformError> {
        Ok(vec![
            GpuAdapter {
                name: "NVIDIA GeForce RTX 4050 Laptop GPU".into(),
                vendor_id: 0x10DE,
                device_id: 0x28A1,
                dedicated_vram_mb: *self.vram_mb.lock().unwrap(),
                shared_memory_mb: 7_900,
                software: false,
            },
            GpuAdapter {
                name: "Microsoft Basic Render Driver".into(),
                vendor_id: 0x1414,
                device_id: 0x8C,
                dedicated_vram_mb: 0,
                shared_memory_mb: 7_900,
                software: true,
            },
        ])
    }
    fn npu(&self) -> Result<Option<String>, PlatformError> {
        Ok(None)
    }
    fn power_status(&self) -> Result<PowerStatus, PlatformError> {
        Ok(*self.power.lock().unwrap())
    }
    fn audio_endpoints(&self) -> Result<Vec<AudioEndpoint>, PlatformError> {
        Ok(vec![AudioEndpoint {
            name: "Mikrofon (Realtek)".into(),
            direction: AudioDirection::Capture,
        }])
    }
    fn machine_seed(&self) -> Result<Option<String>, PlatformError> {
        Ok(Some("3F2504E0-4F89-11D3-9A0C-0305E82C3301".into()))
    }
}

struct Screen(bool);

impl WindowPort for Screen {
    fn list(&self) -> Vec<WindowInfo> {
        vec![WindowInfo {
            id: WindowId(1),
            title: "Gra".into(),
            process: "game.exe".into(),
            focused: true,
            fullscreen: self.0,
        }]
    }
    fn focus(&self, _id: WindowId) -> Result<(), PlatformError> {
        Ok(())
    }
}

fn config(dir: &tempfile::TempDir) -> DeviceProfileConfig {
    DeviceProfileConfig {
        state_dir: dir.path().to_path_buf(),
        overlay: MachineOverlay::default(),
    }
}

fn service(probe: Arc<LaptopProbe>, dir: &tempfile::TempDir) -> DeviceProfileService {
    DeviceProfileService::detect(probe, Some(Arc::new(Screen(true))), config(dir)).unwrap()
}

#[test]
fn contract_suite_on_impl() {
    let dir = tempfile::tempdir().unwrap();
    contract_tests::run_all(|| service(Arc::new(LaptopProbe::new()), &dir));
}

#[test]
fn detection_maps_laptop_and_id_is_stable() {
    let dir = tempfile::tempdir().unwrap();
    let probe = Arc::new(LaptopProbe::new());
    let a = service(Arc::clone(&probe), &dir);
    let b = service(probe, &dir);
    let profile = a.current();
    assert_eq!(profile.machine_id, b.current().machine_id);
    assert_eq!(
        profile.machine_id,
        derive_machine_id("3f2504e0-4f89-11d3-9a0c-0305e82c3301").unwrap()
    );
    assert_eq!(profile.gpus.len(), 1, "adapter programowy pominięty");
    assert_eq!(profile.cpu.l3_cache_mb, Some(24));
    assert_eq!(profile.audio.as_ref().map(Vec::len), Some(1));
    assert!(profile.battery.is_some());
    let rec = a.recommend();
    assert_eq!(
        (rec.class, rec.voice_profile),
        (HwClass::LaptopCuda, VoiceProfile::D)
    );
    assert!(a.fullscreen_active());
    a.set_overlay(MachineOverlay {
        voice_profile: VoiceChoice::B,
        ..MachineOverlay::default()
    });
    assert_eq!(a.recommend().voice_profile, VoiceProfile::B);
}

#[tokio::test]
async fn module_lifecycle_and_events() {
    let dir = tempfile::tempdir().unwrap();
    let probe = Arc::new(LaptopProbe::new());
    let mut svc = service(Arc::clone(&probe), &dir);
    assert_eq!(svc.health(), HealthStatus::NotStarted);
    assert_eq!(svc.stop().await, Err(ModuleError::NotStarted));
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(svc.manifest().id.clone(), Arc::new(bus.clone()));
    svc.start(ctx.clone()).await.unwrap();
    assert_eq!(svc.start(ctx).await, Err(ModuleError::AlreadyStarted));
    assert_eq!(svc.health(), HealthStatus::Healthy);
    let detected = bus.recorded_of_kind(&event_kind(EVENT_DETECTED));
    assert_eq!(detected.len(), 1);
    assert_eq!(detected[0].payload["class"], "laptop-cuda");

    *probe.power.lock().unwrap() = PowerStatus {
        ac_online: Some(false),
        battery_present: true,
        battery_percent: Some(70),
    };
    *probe.vram_mb.lock().unwrap() = 7_936;
    assert!(svc.refresh().unwrap());
    let events = svc.poll_changes().await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DeviceEvent::Changed { .. }))
    );
    assert!(events.contains(&DeviceEvent::PowerChanged {
        power: PowerState::Battery { percent: Some(70) }
    }));
    assert!(events.contains(&DeviceEvent::FullscreenChanged { active: true }));
    assert_eq!(
        bus.recorded_of_kind(&event_kind(EVENT_POWER_CHANGED)).len(),
        1
    );
    assert!(svc.recommend().power_saving);
    assert!(
        svc.poll_changes().await.is_empty(),
        "bez zmian — bez zdarzeń"
    );
    svc.stop().await.unwrap();
    assert_eq!(svc.health(), HealthStatus::NotStarted);
}

#[test]
fn manifest_is_valid_and_matches_crate() {
    let dir = tempfile::tempdir().unwrap();
    let svc = service(Arc::new(LaptopProbe::new()), &dir);
    let m = svc.manifest();
    assert_eq!(m.id.as_str(), "device-profile");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.lifecycle, Lifecycle::Always);
    assert!(MODULE_TOML.contains("platform-contract@1"));
    assert!(format!("{svc:?}").contains("DeviceProfileService"));
}

#[cfg(not(windows))]
#[test]
fn native_probe_detects_this_machine_quickly() {
    let dir = tempfile::tempdir().unwrap();
    let started = std::time::Instant::now();
    let svc = DeviceProfileService::detect_native(config(&dir)).unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_millis(500),
        "SPEC: ≤ 500 ms"
    );
    let p = svc.current();
    assert!(p.cpu.logical_cores >= 1 && p.ram_mb > 0);
    assert!(p.gpus.is_empty() && p.audio.is_none());
    let again = DeviceProfileService::detect_native(config(&dir)).unwrap();
    assert_eq!(again.current().machine_id, p.machine_id);
    assert_eq!(svc.recommend().class, HwClass::Unknown);
}
