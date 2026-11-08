//! Karta dzierżawy Whispera (C6.6) na laptopie właściciela: Intel UHD (karta 0, pamięć z RAM) i
//! RTX 4050 6 GB (karta 1, główna). STT na CUDA zajmuje pulę karty 1 obok modelu rozmowy na
//! tej samej karcie; pula karty zintegrowanej zostaje pusta. Na procesorze — bez karty.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::FakeLauncher;
use device_profile_contract::{Backend, GpuInfo, GpuVendor, Profile, fixtures, recommend};
use model_residency_contract::{
    Budget, Device, GpuId, GpuLayout, LeaseRequest, ModelRole, Placement, Priority, Residency,
};
use model_residency_fake::FakeResidency;
use voice_audio_contract::synth::{SpeechParams, synthetic_speech};
use voice_audio_contract::{Frame, MediaTime};
use voice_stt_contract::{Stt, UtteranceId};
use voice_stt_impl::{SidecarBinaries, WhisperServerConfig, WhisperStt};

const UHD: GpuId = GpuId(0);
const RTX: GpuId = GpuId(1);

fn laptop() -> Profile {
    Profile {
        gpus: vec![
            GpuInfo::describe(0, "Intel(R) UHD Graphics", GpuVendor::Intel, 128, 7_900)
                .with_hardware_flag(Some(true)),
            GpuInfo::describe(
                1,
                "NVIDIA GeForce RTX 4050 Laptop GPU",
                GpuVendor::Nvidia,
                6_141,
                7_900,
            )
            .with_hardware_flag(Some(false)),
        ],
        ..fixtures::laptop()
    }
}

fn residency(profile: &Profile) -> Arc<FakeResidency> {
    let budget = Budget::from_device(&recommend(profile).residency);
    Arc::new(FakeResidency::with_gpus(
        budget,
        GpuLayout::from_profile(profile),
    ))
}

fn config() -> WhisperServerConfig {
    let bins = SidecarBinaries {
        vulkan: None,
        cuda: Some("whisper-server-cuda".into()),
        cpu: "whisper-server".into(),
    };
    let mut cfg = WhisperServerConfig::new(bins, "ggml-large-v3-turbo-q5_0.bin".into());
    cfg.startup_timeout = Duration::from_secs(5);
    cfg.request_timeout = Duration::from_secs(5);
    cfg
}

async fn utterance(stt: &WhisperStt, id: u64) -> Option<Backend> {
    let id = UtteranceId(id);
    stt.start_utterance(id).await.unwrap();
    for (i, c) in synthetic_speech(16_000, 1.0, SpeechParams::default())
        .chunks(160)
        .enumerate()
    {
        let frame = Frame::mono(c.to_vec(), 16_000, MediaTime::from_ms(10 * i as u64));
        stt.push(id, &frame).await.unwrap();
    }
    stt.end_utterance(id).await.unwrap().backend
}

fn llm(vram_mb: u32) -> LeaseRequest {
    LeaseRequest {
        owner: "providers-local".into(),
        model: "bielik-1.5b-v3.0-instruct-q8_0".into(),
        role: ModelRole::Llm,
        priority: Priority::Conversation,
        placement: Placement::GpuPreferred,
        vram_mb,
        ram_mb: 512,
        cpu_ram_mb: 2_700,
        idle_unload_ms: 600_000,
        gpu: Some(RTX),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn whisper_on_cuda_leases_the_rtx_next_to_the_chat_model() {
    let profile = laptop();
    let residency = residency(&profile);
    let chat = residency.acquire(llm(2_456)).unwrap().lease;
    assert_eq!((chat.device, chat.gpu), (Device::Gpu, Some(RTX)));
    let stt = WhisperStt::new(config(), Arc::new(FakeLauncher::default()), Backend::Cuda)
        .unwrap()
        .with_residency(residency.clone())
        .with_cards(&profile);
    assert_eq!(utterance(&stt, 1).await, Some(Backend::Cuda));
    let snap = residency.snapshot();
    let lease = snap
        .leases
        .iter()
        .find(|l| l.request.owner == "voice-stt")
        .unwrap();
    assert_eq!(
        (lease.request.gpu, lease.device, lease.gpu),
        (Some(RTX), Device::Gpu, Some(RTX))
    );
    assert_eq!(snap.gpu(RTX).map(|g| g.used_mb), Some(2_456 + 1_500));
    assert_eq!(
        snap.gpu(UHD).map(|g| g.used_mb),
        Some(0),
        "karta zintegrowana wolna"
    );
    assert!(snap.within_budget());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn whisper_on_cpu_and_without_profile_keeps_the_old_lease() {
    let profile = laptop();
    let residency = residency(&profile);
    // Procesor (np. wybór w Ustawieniach): bez karty w żądaniu.
    let cpu = WhisperStt::new(config(), Arc::new(FakeLauncher::default()), Backend::Cpu)
        .unwrap()
        .with_residency(residency.clone())
        .with_cards(&profile);
    assert_eq!(utterance(&cpu, 1).await, Some(Backend::Cpu));
    let snap = residency.snapshot();
    assert_eq!(snap.leases.len(), 1);
    assert_eq!(
        (snap.leases[0].request.gpu, snap.leases[0].gpu),
        (None, None)
    );
    drop(cpu);
    // Bez profilu: karta główna jak dotąd (żądanie bez karty, dzierżawa na RTX — głównej).
    let plain = WhisperStt::new(config(), Arc::new(FakeLauncher::default()), Backend::Cuda)
        .unwrap()
        .with_residency(residency.clone());
    assert_eq!(utterance(&plain, 2).await, Some(Backend::Cuda));
    let lease = residency.snapshot().leases[0].clone();
    assert_eq!((lease.request.gpu, lease.gpu), (None, Some(RTX)));
}
