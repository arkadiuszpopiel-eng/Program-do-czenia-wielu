//! Karta dzierżawy głosu (C6.6): Supertonic przez DirectML na karcie B (`--dml 1`) zajmuje pulę
//! tej karty i nie blokuje modelu rozmowy na karcie A; głos na procesorze — bez karty.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use model_residency_contract::{
    Budget, Device, GpuId, GpuLayout, GpuPool, LeaseRequest, ModelRole, Placement, Priority,
    Residency,
};
use model_residency_fake::FakeResidency;
use voice_tts_impl::{TtsLease, tts_lease_request};

const A: GpuId = GpuId(0);
const B: GpuId = GpuId(1);

/// Dwie karty AMD po 8 GB (A główna).
fn two_cards() -> Arc<FakeResidency> {
    Arc::new(FakeResidency::with_gpus(
        Budget {
            vram_mb: 7_408,
            ram_mb: 16_384,
            desktop_reserve_mb: 768,
            stt_tts_exclusive: false,
        },
        GpuLayout {
            primary: A,
            others: vec![GpuPool {
                gpu: B,
                vram_mb: 7_408,
                desktop_reserve_mb: 768,
                stt_tts_exclusive: false,
                shared: false,
            }],
        },
    ))
}

#[test]
fn dml_voice_on_card_b_leaves_card_a_for_chat() {
    let residency = two_cards();
    let voice = TtsLease::acquire_request(
        residency.clone(),
        tts_lease_request("supertonic-3", 900, 300_000, Some(1), 1_200),
    )
    .unwrap();
    // Model rozmowy wypełnia całą kartę A — głos na karcie B nie przeszkadza.
    let chat = residency
        .acquire(LeaseRequest {
            owner: "providers-local".into(),
            model: "bielik-11b".into(),
            role: ModelRole::Llm,
            priority: Priority::Conversation,
            placement: Placement::GpuOnly,
            vram_mb: 7_408,
            ram_mb: 512,
            cpu_ram_mb: 12_000,
            idle_unload_ms: 0,
            gpu: Some(A),
        })
        .unwrap();
    assert!(chat.evicted.is_empty(), "{:?}", chat.evicted);
    let snap = residency.snapshot();
    let tts = snap.leases.iter().find(|l| l.id == voice.id()).unwrap();
    assert_eq!((tts.device, tts.gpu), (Device::Gpu, Some(B)));
    assert_eq!(snap.gpu(B).map(|g| g.used_mb), Some(1_200));
    assert_eq!(snap.gpu(A).map(|g| g.used_mb), Some(7_408));
    assert!(snap.within_budget());
}

#[test]
fn cpu_voice_has_no_card() {
    let request = tts_lease_request("pocket-tts-pl", 800, 300_000, None, 1_200);
    assert_eq!(
        (request.placement, request.vram_mb, request.gpu),
        (Placement::CpuOnly, 0, None)
    );
    let residency = two_cards();
    let lease = TtsLease::acquire(residency.clone(), "pocket-tts-pl", 800, 300_000).unwrap();
    let snap = residency.snapshot();
    let l = snap.leases.iter().find(|l| l.id == lease.id()).unwrap();
    assert_eq!(
        (l.device, l.gpu, l.request.cpu_ram_mb),
        (Device::Cpu, None, 800)
    );
}
