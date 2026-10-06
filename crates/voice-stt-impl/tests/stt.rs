//! Testy `WhisperStt` na udawanym `whisper-server`: kontrakt, pola żądań, start z 503, awaria GPU
//! → fallback CPU bez utraty wypowiedzi, dzierżawa rezydencji, moduł.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::FakeLauncher;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use device_profile_contract::Backend;
use model_residency_contract::{
    Budget, Device, LeaseRequest, ModelRole, Placement, Priority, Residency,
};
use voice_audio_contract::synth::{SpeechParams, synthetic_speech};
use voice_audio_contract::{Frame, MediaTime};
use voice_stt_contract::{Health, Stt, SttEvent, UtteranceId, contract_tests, event_kind};
use voice_stt_impl::{
    MODULE_TOML, SidecarBinaries, VoiceSttModule, WhisperServerConfig, WhisperStt,
};

fn config() -> WhisperServerConfig {
    let bins = SidecarBinaries {
        vulkan: Some("whisper-server-vulkan".into()),
        cuda: None,
        cpu: "whisper-server".into(),
    };
    let mut cfg = WhisperServerConfig::new(bins, "ggml-large-v3-turbo-q5_0.bin".into());
    cfg.startup_timeout = Duration::from_secs(5);
    cfg.request_timeout = Duration::from_secs(5);
    cfg
}

async fn speak(
    stt: &WhisperStt,
    id: UtteranceId,
    secs: f32,
) -> Vec<voice_stt_contract::Transcript> {
    stt.start_utterance(id).await.unwrap();
    let mut partials = Vec::new();
    for (i, c) in synthetic_speech(16_000, secs, SpeechParams::default())
        .chunks(160)
        .enumerate()
    {
        if let Some(p) = stt
            .push(
                id,
                &Frame::mono(c.to_vec(), 16_000, MediaTime::from_ms(10 * i as u64)),
            )
            .await
            .unwrap()
        {
            partials.push(p);
        }
    }
    partials
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_suite() {
    let stt =
        WhisperStt::new(config(), Arc::new(FakeLauncher::default()), Backend::Vulkan).unwrap();
    contract_tests::run_all(&stt).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn requests_carry_language_beam_prompt_and_wav() {
    let launcher = Arc::new(FakeLauncher {
        health_503: 3,
        ..FakeLauncher::default()
    });
    let stt = WhisperStt::new(config(), launcher.clone(), Backend::Vulkan).unwrap();
    assert_eq!(stt.health(), Health::Stopped);
    let partials = speak(&stt, UtteranceId(1), 2.2).await;
    assert_eq!(partials.len(), 2, "partial co 1 s");
    assert!(!partials[0].is_final);
    let t = stt.end_utterance(UtteranceId(1)).await.unwrap();
    assert_eq!(t.text, "Delta, otwórz plik.");
    assert_eq!(t.lang, "pl");
    assert_eq!(t.backend, Some(Backend::Vulkan));
    assert_eq!(stt.health(), Health::Ready(Backend::Vulkan));
    let reqs = launcher.shared.requests.lock().unwrap().clone();
    assert_eq!(reqs.len(), 3);
    assert!(
        reqs[0].contains("name=\"beam_size\"\r\n\r\n1"),
        "partial zachłanny"
    );
    assert!(
        reqs[2].contains("name=\"beam_size\"\r\n\r\n5"),
        "final z wiązką"
    );
    assert!(reqs[2].contains("name=\"language\"\r\n\r\nauto"));
    assert!(
        reqs[2].contains("Alfa, Beta, Gama, Delta."),
        "hotwords w prompt"
    );
    assert!(reqs[2].contains("RIFF") && reqs[2].contains("verbose_json"));
    let launches = launcher.launches.lock().unwrap().clone();
    assert_eq!(launches.len(), 1, "sidecar uruchomiony raz");
    assert_eq!(launches[0].program.to_str(), Some("whisper-server-vulkan"));
    assert!(
        launcher
            .shared
            .health_polls
            .load(std::sync::atomic::Ordering::SeqCst)
            >= 4
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gpu_crash_falls_back_to_cpu_without_losing_utterance() {
    let launcher = Arc::new(FakeLauncher {
        crash: vec![Backend::Vulkan],
        ..FakeLauncher::default()
    });
    let stt = WhisperStt::new(config(), launcher.clone(), Backend::Vulkan).unwrap();
    let mut cfg = voice_stt_contract::SttCfg::default();
    cfg.two_pass.enabled = false;
    stt.configure(cfg).await.unwrap();
    speak(&stt, UtteranceId(9), 1.5).await;
    let t = stt.end_utterance(UtteranceId(9)).await.unwrap();
    assert_eq!(t.text, "Delta, otwórz plik.");
    assert_eq!(t.backend, Some(Backend::Cpu));
    let ev = stt.take_events();
    assert!(ev.iter().any(|e| matches!(e, SttEvent::BackendFallback { from: Backend::Vulkan, to: Backend::Cpu, reason } if reason.contains("ErrorDeviceLost"))));
    let launches = launcher.launches.lock().unwrap().clone();
    assert_eq!(
        launches.iter().map(|l| l.backend).collect::<Vec<_>>(),
        vec![Backend::Vulkan, Backend::Cpu]
    );
    assert!(launches[1].args.contains(&"-ng".to_owned()));
    assert!(matches!(stt.health(), Health::Degraded(_)));
    // Kolejne wypowiedzi zostają na CPU (backend oznaczony jako niesprawny).
    speak(&stt, UtteranceId(10), 1.0).await;
    assert_eq!(
        stt.end_utterance(UtteranceId(10)).await.unwrap().backend,
        Some(Backend::Cpu)
    );
    assert_eq!(launcher.launches.lock().unwrap().len(), 2);
}

/// Fala 6 (laptop z RTX 4050): kompilacja CUDA, która nie startuje (brak sterownika albo
/// bibliotek `cudart`), nie psuje rozpoznawania — ta sama wypowiedź idzie na CPU.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gpu_start_failure_falls_back_to_cpu() {
    let launcher = Arc::new(FakeLauncher {
        dead_on_start: vec![Backend::Vulkan],
        ..FakeLauncher::default()
    });
    let stt = WhisperStt::new(config(), launcher.clone(), Backend::Vulkan).unwrap();
    let mut cfg = voice_stt_contract::SttCfg::default();
    cfg.two_pass.enabled = false;
    stt.configure(cfg).await.unwrap();
    speak(&stt, UtteranceId(3), 1.5).await;
    let t = stt.end_utterance(UtteranceId(3)).await.unwrap();
    assert_eq!(t.text, "Delta, otwórz plik.");
    assert_eq!(t.backend, Some(Backend::Cpu));
    assert!(stt.take_events().iter().any(|e| matches!(e, SttEvent::BackendFallback { from: Backend::Vulkan, to: Backend::Cpu, reason } if reason.contains("nie wystartował"))));
    assert!(matches!(stt.health(), Health::Degraded(_)));
    speak(&stt, UtteranceId(4), 1.0).await;
    stt.end_utterance(UtteranceId(4)).await.unwrap();
    let backends: Vec<_> = launcher
        .launches
        .lock()
        .unwrap()
        .iter()
        .map(|l| l.backend)
        .collect();
    assert_eq!(
        backends,
        [Backend::Vulkan, Backend::Cpu],
        "GPU nie jest ponawiane"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn residency_lease_and_cpu_placement() {
    let residency = Arc::new(model_residency_fake::FakeResidency::new(Budget {
        vram_mb: 0,
        ram_mb: 8_000,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: false,
    }));
    let launcher = Arc::new(FakeLauncher::default());
    let stt = WhisperStt::new(config(), launcher.clone(), Backend::Vulkan)
        .unwrap()
        .with_residency(residency.clone());
    speak(&stt, UtteranceId(1), 1.0).await;
    let t = stt.end_utterance(UtteranceId(1)).await.unwrap();
    assert_eq!(
        t.backend,
        Some(Backend::Cpu),
        "brak VRAM → dzierżawa na CPU"
    );
    let snap = residency.snapshot();
    assert_eq!(snap.leases.len(), 1);
    assert_eq!(snap.leases[0].request.owner, "voice-stt");
    drop(stt);
    assert!(residency.snapshot().leases.is_empty());
}

/// Fala 6, laptop RTX 4050 6 GB (budżet 5153 MB): dzierżawa LLM `providers-local` jak dla
/// Bielika 4.5B Q8_0 (wagi + KV cache części warstw na karcie). STT nie wypiera modelu rozmowy
/// z karty: gdy whisper CUDA się nie mieści (51 warstw LLM, bez rezerwy) — CPU przez całą rozmowę
/// (bez przeładowań LLM przy każdej wypowiedzi); gdy LLM zostawia rezerwę (34 warstwy) — CUDA obok.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stt_never_evicts_the_conversation_model_on_a_6_gb_laptop() {
    for (llm_vram, expected) in [(5_080, Backend::Cpu), (3_570, Backend::Cuda)] {
        let residency = Arc::new(model_residency_fake::FakeResidency::new(Budget {
            vram_mb: 5_921 - 768,
            ram_mb: 8_000,
            desktop_reserve_mb: 768,
            stt_tts_exclusive: true,
        }));
        let llm = residency
            .acquire(LeaseRequest {
                owner: "providers-local".into(),
                model: "bielik-4.5b-v3.0-instruct-q8_0".into(),
                role: ModelRole::Llm,
                priority: Priority::Conversation,
                placement: Placement::GpuPreferred,
                vram_mb: llm_vram,
                ram_mb: 3_146,
                cpu_ram_mb: 6_080,
                idle_unload_ms: 600_000,
            })
            .unwrap()
            .lease;
        let mut cfg = config();
        cfg.binaries.cuda = Some("whisper-server-cuda".into());
        let launcher = Arc::new(FakeLauncher::default());
        let stt = WhisperStt::new(cfg, launcher.clone(), Backend::Cuda)
            .unwrap()
            .with_residency(residency.clone());
        for id in 1..=3 {
            speak(&stt, UtteranceId(id), 1.0).await;
            let t = stt.end_utterance(UtteranceId(id)).await.unwrap();
            assert_eq!(
                t.backend,
                Some(expected),
                "LLM {llm_vram} MB, wypowiedź {id}"
            );
        }
        assert_eq!(launcher.launches.lock().unwrap().len(), 1, "sidecar raz");
        let snap = residency.snapshot();
        let lease = snap.leases.iter().find(|l| l.id == llm.id);
        assert_eq!(lease.map(|l| l.device), Some(Device::Gpu), "LLM na karcie");
        assert!(snap.used.vram_mb <= snap.budget.vram_mb);
    }
}

#[tokio::test]
async fn module_publishes_events() {
    let mut m = VoiceSttModule::new().unwrap();
    assert!(MODULE_TOML.contains("isolation = \"process\""));
    let ev = [SttEvent::ModelUnloaded {
        reason: "idle".into(),
    }];
    assert_eq!(m.publish(&ev).await, 0);
    let bus = FakeBus::default();
    m.start(ModuleContext::new(
        m.manifest().id.clone(),
        Arc::new(bus.clone()),
    ))
    .await
    .unwrap();
    assert_eq!(m.health(), HealthStatus::Healthy);
    assert_eq!(m.publish(&ev).await, 1);
    assert_eq!(
        bus.recorded_of_kind(&event_kind("voice.stt.model.unloaded"))
            .len(),
        1
    );
    m.stop().await.unwrap();
    assert_eq!(m.health(), HealthStatus::NotStarted);
}

/// Wolny backend (CPU: kilka sekund na przebieg whispera): kolejny partial dopiero po co najmniej
/// dwukrotności czasu poprzedniego — rozpoznawanie nie zajmuje całego czasu mowy, a final nie czeka
/// za kolejką partiali. Szybki backend (karta) — odstęp bez zmian (`requests_carry_…`: co 1 s).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_partials_back_off_so_they_never_hog_the_engine() {
    let launcher = Arc::new(FakeLauncher::default());
    launcher
        .shared
        .inference_delay_ms
        .store(1_200, std::sync::atomic::Ordering::SeqCst);
    let stt = WhisperStt::new(config(), launcher.clone(), Backend::Vulkan).unwrap();
    // 4,2 s mowy: co 1 s byłyby 4 partiale; po partialu 1,2 s odstęp ≥ 2,4 s → 2 (przy 1,0 i 3,4 s).
    let partials = speak(&stt, UtteranceId(1), 4.2).await;
    assert_eq!(partials.len(), 2);
    let t = stt.end_utterance(UtteranceId(1)).await.unwrap();
    assert_eq!(t.text, "Delta, otwórz plik.");
}
