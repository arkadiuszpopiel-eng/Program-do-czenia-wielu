//! Pule kart w praktyce (C6.6): laptop właściciela (Intel UHD — karta 0, RTX 4050 — karta 1,
//! główna) z kartami wybranymi tak jak silniki, dwie karty AMD (mowa na karcie B nie blokuje
//! rozmowy na karcie A), zmiana sprzętu przelicza pule (wprost i przez zdarzenie `device.changed`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use core_bus_contract::{Event, EventBus, Level};
use core_bus_fake::FakeBus;
use core_registry_contract::{Module, ModuleContext};
use device_profile_contract::{
    Backend, DeviceProfile, EVENT_CHANGED, GpuInfo, GpuVendor, Profile, event_kind, fixtures,
};
use device_profile_fake::FakeDeviceProfile;
use model_residency_contract::{
    Change, Device, GpuId, LeaseRequest, ModelRole, Placement, Priority, Residency, RevokeReason,
    card_for,
};
use model_residency_impl::{ResidencyConfig, ResidencyManager, ResidencyModule};

const UHD: GpuId = GpuId(0);
const RTX: GpuId = GpuId(1);

fn card(id: u32, name: &str, vendor: GpuVendor, vram_mb: u32, integrated: bool) -> GpuInfo {
    GpuInfo::describe(id, name, vendor, vram_mb, 7_900).with_hardware_flag(Some(integrated))
}

fn laptop() -> Profile {
    Profile {
        gpus: vec![
            card(0, "Intel(R) UHD Graphics", GpuVendor::Intel, 128, true),
            card(
                1,
                "NVIDIA GeForce RTX 4050 Laptop GPU",
                GpuVendor::Nvidia,
                6_141,
                false,
            ),
        ],
        ..fixtures::laptop()
    }
}

/// Dwie osobne karty AMD po 16 GB: A (adapter 0, główna) i B (adapter 1).
fn two_amd() -> Profile {
    Profile {
        gpus: vec![
            card(0, "AMD Radeon RX 9070 XT", GpuVendor::Amd, 16_368, false),
            card(1, "AMD Radeon RX 7800 XT", GpuVendor::Amd, 16_304, false),
        ],
        ..fixtures::desktop()
    }
}

fn lease(
    owner: &str,
    role: ModelRole,
    priority: Priority,
    placement: Placement,
    vram_mb: u32,
    gpu: Option<GpuId>,
) -> LeaseRequest {
    LeaseRequest {
        owner: owner.into(),
        model: format!("{owner}-model"),
        role,
        priority,
        placement,
        vram_mb,
        ram_mb: 300,
        cpu_ram_mb: 1_500,
        idle_unload_ms: 0,
        gpu,
    }
}

fn manager(device: &Arc<FakeDeviceProfile>) -> ResidencyManager {
    ResidencyManager::from_device(device.clone(), &ResidencyConfig::default())
}

#[test]
fn laptop_stt_on_cuda_and_chat_lease_the_rtx_while_vad_stays_on_cpu() {
    let profile = laptop();
    let m = manager(&Arc::new(FakeDeviceProfile::new(profile.clone())));
    let state = m.snapshot();
    assert_eq!(state.gpus.first().map(|g| g.pool.gpu), Some(RTX), "główna");
    assert!(
        state.gpu(UHD).is_some_and(|g| g.pool.shared),
        "UHD — pamięć z RAM"
    );
    // Karty jak w silnikach: whisper na CUDA i model rozmowy — RTX.
    let cuda = card_for(&profile, Backend::Cuda, None);
    assert_eq!(cuda, Some(RTX));
    let stt = m
        .acquire(lease(
            "voice-stt",
            ModelRole::Stt,
            Priority::VoiceRt,
            Placement::GpuIfFree,
            1_500,
            cuda,
        ))
        .unwrap()
        .lease;
    let chat = m
        .acquire(lease(
            "providers-local",
            ModelRole::Llm,
            Priority::Conversation,
            Placement::GpuPreferred,
            2_456,
            cuda,
        ))
        .unwrap();
    assert!(chat.evicted.is_empty());
    let vad = m
        .acquire(lease(
            "voice-vad",
            ModelRole::Vad,
            Priority::VoiceRt,
            Placement::CpuOnly,
            0,
            None,
        ))
        .unwrap()
        .lease;
    assert_eq!((stt.device, stt.gpu), (Device::Gpu, Some(RTX)));
    assert_eq!(
        (chat.lease.device, chat.lease.gpu),
        (Device::Gpu, Some(RTX))
    );
    assert_eq!((vad.device, vad.gpu), (Device::Cpu, None));
    let state = m.snapshot();
    assert_eq!(state.gpu(RTX).map(|g| g.used_mb), Some(1_500 + 2_456));
    assert_eq!(state.gpu(UHD).map(|g| g.used_mb), Some(0));
    assert!(state.within_budget());
}

#[test]
fn speech_on_card_b_does_not_block_chat_on_card_a() {
    let profile = two_amd();
    let m = manager(&Arc::new(FakeDeviceProfile::new(profile.clone())));
    let a = card_for(&profile, Backend::Vulkan, None);
    assert_eq!(a, Some(GpuId(0)));
    let b = card_for(&profile, Backend::Vulkan, Some("AMD Radeon RX 7800 XT"));
    assert_eq!(b, Some(GpuId(1)));
    let pool_a = m.snapshot().budget.vram_mb;
    let pool_b = m.snapshot().gpu(GpuId(1)).map(|g| g.pool.vram_mb).unwrap();
    // Mowa (STT + TTS) w użyciu na karcie B zajmuje całą jej pulę.
    for (owner, role, mb) in [
        ("voice-stt", ModelRole::Stt, 3_000),
        ("voice-tts", ModelRole::Tts, pool_b - 3_000),
    ] {
        let grant = m
            .acquire(lease(
                owner,
                role,
                Priority::VoiceRt,
                Placement::GpuOnly,
                mb,
                b,
            ))
            .unwrap();
        m.set_in_use(grant.lease.id, true).unwrap();
        assert_eq!(grant.lease.gpu, b);
    }
    // Rozmowa na karcie A — cała jej pula, bez czekania na mowę i bez eksmisji.
    let chat = m
        .acquire(lease(
            "providers-local",
            ModelRole::Llm,
            Priority::Conversation,
            Placement::GpuOnly,
            pool_a,
            a,
        ))
        .unwrap();
    assert!(chat.evicted.is_empty(), "{:?}", chat.evicted);
    assert_eq!((chat.lease.device, chat.lease.gpu), (Device::Gpu, a));
    assert_eq!(m.snapshot().leases.len(), 3);
    assert!(m.snapshot().within_budget());
}

#[test]
fn device_change_relayouts_the_pools() {
    let config = ResidencyConfig::default();
    let device = Arc::new(FakeDeviceProfile::new(laptop()));
    let m = manager(&device);
    assert!(
        m.sync_hardware(device.as_ref(), &config).is_none(),
        "bez zmian"
    );
    let light = m
        .acquire(lease(
            "local-light",
            ModelRole::Llm,
            Priority::Background,
            Placement::GpuPreferred,
            1_000,
            Some(UHD),
        ))
        .unwrap()
        .lease;
    assert_eq!(light.gpu, Some(UHD));
    let stt = m
        .acquire(lease(
            "voice-stt",
            ModelRole::Stt,
            Priority::VoiceRt,
            Placement::GpuIfFree,
            1_500,
            Some(RTX),
        ))
        .unwrap()
        .lease;
    let before = m.snapshot().budget;
    // Karta zintegrowana wyłączona w BIOS-ie, RTX zostaje (ten sam adapter 1).
    let mut next = laptop();
    next.gpus.retain(|g| g.id == 1);
    device.hot_plug(next);
    assert!(device.refresh().unwrap());
    let changes = m.sync_hardware(device.as_ref(), &config).unwrap();
    let revoked: Vec<_> = changes
        .iter()
        .filter_map(|c| match c {
            Change::Revoked(r) => Some(r.lease.id),
            Change::Moved(_) => None,
        })
        .collect();
    assert_eq!(revoked, [light.id], "dzierżawa na karcie, której nie ma");
    assert!(changes.iter().all(|c| !matches!(c, Change::Revoked(r)
        if r.reason == RevokeReason::Idle)));
    let state = m.snapshot();
    assert_eq!(state.gpus.len(), 1, "{:?}", state.gpus);
    assert_eq!(state.gpus[0].pool.gpu, RTX);
    assert_eq!(state.budget, before, "karta główna bez zmian");
    assert!(m.lease(stt.id).is_some_and(|l| l.gpu == Some(RTX)));
    assert!(
        m.sync_hardware(device.as_ref(), &config).is_none(),
        "drugi raz bez zmian"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn module_relayouts_pools_on_device_changed() {
    let config = ResidencyConfig::default();
    let device = Arc::new(FakeDeviceProfile::new(fixtures::desktop()));
    let manager = Arc::new(manager(&device));
    assert_eq!(manager.snapshot().gpus.len(), 1);
    let mut module = ResidencyModule::new(manager.clone(), None, Duration::from_secs(3600))
        .unwrap()
        .with_hardware(device.clone(), config);
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    // Pierwszy krok zadania tła już minął (bez zmian); druga karta podłączona w trakcie pracy.
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(manager.snapshot().gpus.len(), 1);
    device.hot_plug(two_amd());
    assert!(device.refresh().unwrap());
    let payload = serde_json::json!({ "event": "changed" });
    bus.publish(Event::new(event_kind(EVENT_CHANGED), Level::Info, payload))
        .await
        .unwrap();
    let mut cards = 0;
    for _ in 0..200 {
        cards = manager.snapshot().gpus.len();
        if cards == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(cards, 2, "pule po zdarzeniu device.changed");
    module.stop().await.unwrap();
}
