//! Testy implementacji: zestaw kontraktowy, budżety z device-profile (baseline/laptop/desktop),
//! sygnały pełnego ekranu i baterii, moduł i zdarzenia na magistrali, konfiguracja.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};
use device_profile_contract::{DeviceProfile, PowerState};
use device_profile_fake::FakeDeviceProfile;
use model_residency_contract::contract_tests::{self, Harness, request};
use model_residency_contract::{
    Budget, Device, ManualClock, Mode, ModelRole, Placement, Priority, Residency, event_kind,
};
use model_residency_impl::{
    Auto, DeviceSignals, MODULE_TOML, ResidencyConfig, ResidencyManager, ResidencyModule, Switch,
    parse_duration,
};

#[derive(Default)]
struct ImplHarness {
    clock: Mutex<Option<Arc<ManualClock>>>,
}

impl Harness for ImplHarness {
    type R = ResidencyManager;

    fn residency(&self, budget: Budget) -> ResidencyManager {
        let clock = Arc::new(ManualClock::new());
        *self.clock.lock().unwrap() = Some(Arc::clone(&clock));
        ResidencyManager::new(budget, clock)
    }

    fn advance_ms(&self, ms: u64) {
        if let Some(c) = self.clock.lock().unwrap().as_ref() {
            c.advance_ms(ms);
        }
    }
}

#[test]
fn contract_suite_on_impl() {
    contract_tests::run_all(&ImplHarness::default());
}

#[test]
fn budgets_follow_device_profile() {
    let config = ResidencyConfig::default();
    let baseline = ResidencyManager::from_device(Arc::new(FakeDeviceProfile::baseline()), &config);
    let b = baseline.snapshot().budget;
    assert_eq!(b.desktop_reserve_mb, 768);
    assert!(b.vram_mb >= 7_000 && b.vram_mb <= 8_192 - 512, "{b:?}");
    let laptop = ResidencyManager::from_device(Arc::new(FakeDeviceProfile::laptop()), &config);
    assert!(
        laptop.snapshot().budget.stt_tts_exclusive,
        "6 GB: STT/TTS na zmianę"
    );
    // Baseline: STT turbo (≈1,5 GB) + LLM 3–4B Q4 (≈3,5 GB) naraz na GPU.
    let stt = baseline.acquire(voice_stt()).unwrap();
    let llm = baseline
        .acquire(request("providers-local", Priority::Conversation, 3_500))
        .unwrap();
    assert_eq!(
        (stt.lease.device, llm.lease.device),
        (Device::Gpu, Device::Gpu)
    );
    // Nadpisanie rezerwy pulpitu i RAM w konfiguracji maszyny.
    let custom = ResidencyConfig::from_toml(
        "vram_mb = \"auto\"\nram_mb = 4000\ndesktop_reserve_mb = 1024\ntick = \"5s\"",
    )
    .unwrap();
    assert_eq!(custom.tick, Duration::from_secs(5));
    let rec = FakeDeviceProfile::baseline().recommend().residency;
    let cb = custom.budget(&rec);
    assert_eq!(cb.ram_mb, 4_000);
    assert_eq!(cb.desktop_reserve_mb, 1_024);
    assert_eq!(cb.vram_mb + 1_024, rec.vram_mb + rec.desktop_reserve_mb);
}

fn voice_stt() -> model_residency_contract::LeaseRequest {
    let mut r = request("voice-stt", Priority::VoiceRt, 1_500);
    r.role = ModelRole::Stt;
    r
}

#[test]
fn fullscreen_and_battery_signals_switch_modes() {
    let device = Arc::new(FakeDeviceProfile::laptop());
    let manager = ResidencyManager::from_device(device.clone(), &ResidencyConfig::default());
    let stt = manager.acquire(voice_stt()).unwrap();
    let mut bg = request("search", Priority::Background, 0);
    bg.placement = Placement::CpuOnly;
    let bg = manager.acquire(bg).unwrap();
    device.set_fullscreen(true);
    device.set_power(PowerState::Battery { percent: Some(40) });
    let signals = DeviceSignals(device.clone() as Arc<dyn DeviceProfile>);
    ResidencyModule::tick_once(&manager, Some(&signals));
    let s = manager.snapshot();
    assert!(s.mode.gaming && s.mode.battery);
    assert_eq!(manager.lease(stt.lease.id).unwrap().device, Device::Cpu);
    assert!(
        manager.lease(bg.lease.id).is_none(),
        "tło wyładowane na baterii"
    );
    assert_eq!(s.used.vram_mb, 0, "gra: 0 dzierżaw GPU");
    // Wyłączony przełącznik gry ignoruje pełny ekran.
    let off = ResidencyConfig {
        gaming_mode: Switch::Off,
        ..ResidencyConfig::default()
    };
    let masked = ResidencyManager::from_device(device, &off);
    assert!(!masked.snapshot().mode.gaming);
    assert!(masked.snapshot().mode.battery);
}

#[tokio::test]
async fn module_publishes_events_in_order() {
    let manager = Arc::new(ResidencyManager::new(
        Budget {
            vram_mb: 2_000,
            ram_mb: 8_000,
            desktop_reserve_mb: 768,
            stt_tts_exclusive: false,
        },
        Arc::new(ManualClock::new()),
    ));
    let mut module =
        ResidencyModule::new(manager.clone(), None, Duration::from_secs(3600)).unwrap();
    assert_eq!(module.health(), HealthStatus::NotStarted);
    assert_eq!(module.stop().await, Err(ModuleError::NotStarted));
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx.clone()).await.unwrap();
    assert_eq!(module.start(ctx).await, Err(ModuleError::AlreadyStarted));
    assert_eq!(module.health(), HealthStatus::Healthy);
    let llm = manager
        .acquire(request("providers-local", Priority::Conversation, 1_800))
        .unwrap();
    manager.acquire(voice_stt()).unwrap();
    let mut huge = request("x", Priority::VoiceRt, 90_000);
    huge.cpu_ram_mb = 90_000;
    assert!(manager.acquire(huge).is_err());
    manager.set_mode(Mode {
        gaming: true,
        ..Mode::normal()
    });
    assert!(manager.lease(llm.lease.id).is_none());
    // Zdarzenia docierają asynchronicznie — czekamy na ostatnie (bez sleep: yield).
    let expected = [
        "residency.granted",
        "residency.evicted",
        "residency.oom_avoided",
        "residency.granted",
        "residency.budget_exceeded",
        "residency.mode_changed",
        "residency.moved",
    ];
    for _ in 0..1_000 {
        if bus.recorded().len() >= expected.len() {
            break;
        }
        tokio::task::yield_now().await;
    }
    let names: Vec<String> = bus
        .recorded()
        .iter()
        .map(|e| e.kind.as_str().to_owned())
        .collect();
    assert_eq!(names, expected);
    let evicted = bus.recorded_of_kind(&event_kind("residency.evicted"));
    assert_eq!(
        evicted[0].payload["revocation"]["reason"]["reason"],
        "preempted"
    );
    module.stop().await.unwrap();
    assert_eq!(module.health(), HealthStatus::NotStarted);
    assert!(MODULE_TOML.contains("model-residency-contract@1"));
    assert_eq!(module.manifest().id.as_str(), "model-residency");
}

#[test]
fn config_parsing_and_durations() {
    assert_eq!(
        ResidencyConfig::from_toml("").unwrap(),
        ResidencyConfig::default()
    );
    let c = ResidencyConfig::from_toml("vram_mb = 5000\ngaming_mode = \"off\"").unwrap();
    assert_eq!(c.vram_mb, Auto::Mb(5_000));
    assert_eq!(c.gaming_mode, Switch::Off);
    for bad in [
        "vram_mb = \"dużo\"",
        "vram_mb = -1",
        "tick = \"5 lat\"",
        "nieznane = 1",
    ] {
        assert!(ResidencyConfig::from_toml(bad).is_err(), "{bad}");
    }
    assert_eq!(parse_duration("250ms"), Some(Duration::from_millis(250)));
    assert_eq!(parse_duration("10m"), Some(Duration::from_secs(600)));
    assert_eq!(parse_duration("1h"), Some(Duration::from_secs(3_600)));
    assert_eq!(parse_duration("h"), None);
    assert_eq!(parse_duration("5d"), None);
}

#[tokio::test(start_paused = true)]
async fn background_tick_reaps_idle_leases() {
    let clock = Arc::new(ManualClock::new());
    let manager = Arc::new(ResidencyManager::new(
        Budget {
            vram_mb: 8_000,
            ram_mb: 8_000,
            desktop_reserve_mb: 768,
            stt_tts_exclusive: false,
        },
        clock.clone(),
    ));
    let lease = manager
        .acquire(request("providers-local", Priority::Conversation, 1_000))
        .unwrap();
    let mut module =
        ResidencyModule::new(manager.clone(), None, Duration::from_millis(100)).unwrap();
    let bus = FakeBus::default();
    module
        .start(ModuleContext::new(
            module.manifest().id.clone(),
            Arc::new(bus),
        ))
        .await
        .unwrap();
    clock.advance_ms(1_000);
    // Czas wirtualny tokio: tyknięcie zadania tła bez prawdziwego czekania.
    tokio::time::advance(Duration::from_millis(150)).await;
    for _ in 0..100 {
        if manager.lease(lease.lease.id).is_none() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(manager.lease(lease.lease.id).is_none());
    module.stop().await.unwrap();
}
