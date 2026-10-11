//! Testy serwisu TTS: kontrakt na udawanym sidecarze Pocket (JSON-lines po `duplex`) i udawanym
//! Piperze, znaczniki natywne/estymowane, fallback, prywatność, cache fraz, stop, rezydencja.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use model_residency_contract::Residency;
use personas_contract::PersonaId;
use providers_contract::PrivacyTag;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use voice_audio_contract::synth::estimate_f0;
use voice_tts_contract::contract_tests::{self, collect, request};
use voice_tts_contract::{
    CancelToken, CloudTts, MarksKind, Tts, TtsEngine, TtsError, TtsEvent, TtsHealth, VoiceRef,
    v0_chains,
};
use voice_tts_impl::cache::PhraseCache;
use voice_tts_impl::engines::{PiperEngine, ProcessRunner, TtsBackend};
use voice_tts_impl::sidecar::PocketSidecar;
use voice_tts_impl::{TtsLease, TtsService, VoiceTtsModule};

fn tone_pcm16(chars: usize, rate: u32) -> Vec<u8> {
    let n = chars * rate as usize * 60 / 1000;
    (0..n)
        .flat_map(|i| {
            let t = i as f32 / rate as f32;
            let s = (1..=5)
                .map(|h| (2.0 * std::f32::consts::PI * 200.0 * h as f32 * t).sin() / h as f32)
                .sum::<f32>()
                * 0.2;
            ((s * 32_767.0) as i16).to_le_bytes()
        })
        .collect()
}

#[derive(Default, Clone)]
struct SidecarCtl {
    fail: Arc<AtomicBool>,
    requests: Arc<AtomicUsize>,
}

/// Udawany sidecar Pocket: odpowiada audio w dwóch kawałkach, natywnymi znacznikami słów i `done`.
fn pocket(ctl: SidecarCtl) -> Arc<dyn TtsBackend> {
    let (client, server) = tokio::io::duplex(1 << 20);
    let (sr, sw) = tokio::io::split(server);
    tokio::spawn(async move {
        let mut lines = BufReader::new(sr).lines();
        let mut w = sw;
        while let Ok(Some(line)) = lines.next_line().await {
            let req: serde_json::Value = serde_json::from_str(&line).unwrap();
            ctl.requests.fetch_add(1, Ordering::SeqCst);
            let id = req["id"].as_u64().unwrap();
            let text = req["text"].as_str().unwrap().to_owned();
            let mut out = String::from("log: synteza\n");
            if ctl.fail.load(Ordering::SeqCst) {
                out += &format!(
                    "{{\"type\":\"error\",\"id\":{id},\"message\":\"model niezaładowany\"}}\n"
                );
            } else {
                let pcm = tone_pcm16(text.chars().count(), 24_000);
                let half = pcm.len() / 4 * 2;
                for part in [&pcm[..half], &pcm[half..]] {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(part);
                    out += &format!(
                        "{{\"type\":\"audio\",\"id\":{id},\"pcm16\":\"{b64}\",\"sample_rate\":24000}}\n"
                    );
                }
                let mut t = 0u32;
                for wd in text.split_whitespace() {
                    let d = wd.chars().count() as u32 * 60;
                    out += &format!(
                        "{{\"type\":\"word\",\"id\":{id},\"word\":{},\"start_ms\":{t},\"end_ms\":{}}}\n",
                        serde_json::json!(wd),
                        t + d
                    );
                    t += d + 60;
                }
                out += &format!("{{\"type\":\"done\",\"id\":{id}}}\n");
            }
            w.write_all(out.as_bytes()).await.unwrap();
        }
    });
    let (cr, cw) = tokio::io::split(client);
    Arc::new(PocketSidecar::with_channel(
        Box::new(BufReader::new(cr)),
        Box::new(cw),
    ))
}

struct FakePiper(Arc<AtomicUsize>);

#[async_trait]
impl ProcessRunner for FakePiper {
    async fn run(
        &self,
        _p: &Path,
        args: &[String],
        stdin: Vec<u8>,
        _t: Duration,
    ) -> Result<Vec<u8>, String> {
        assert!(args.contains(&"--output_raw".to_owned()));
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(tone_pcm16(
            String::from_utf8_lossy(&stdin).trim().chars().count(),
            22_050,
        ))
    }
}

fn piper(count: Arc<AtomicUsize>) -> Arc<dyn TtsBackend> {
    Arc::new(PiperEngine::new(
        "piper".into(),
        "/modele".into(),
        Arc::new(FakePiper(count)),
    ))
}

fn service(ctl: &SidecarCtl) -> TtsService {
    TtsService::new(
        v0_chains(),
        vec![pocket(ctl.clone()), piper(Arc::default())],
    )
    .unwrap()
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(&service(&SidecarCtl::default())).await;
}

#[tokio::test]
async fn native_marks_scaled_by_tempo_and_distinct_pitch() {
    let tts = service(&SidecarCtl::default());
    let delta = collect(
        &tts,
        request(1, PersonaId::delta(), "Lecę dalej teraz."),
        CancelToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(delta[0].marks_kind, MarksKind::Native);
    assert_eq!(delta[0].engine, "pocket");
    // Delta: tempo 1,08 → czasy słów skrócone o 1/1,08 („Lecę” 240 ms + 60 ms przerwy).
    assert_eq!(delta[0].marks[1].start_ms, (300.0f32 / 1.08).round() as u32);
    let alfa = collect(
        &tts,
        request(2, PersonaId::alfa(), "Lecę dalej teraz."),
        CancelToken::new(),
    )
    .await
    .unwrap();
    let f0 = |c: &voice_tts_contract::TtsChunk| {
        estimate_f0(&c.audio.pcm[1_000..5_800], 24_000, 80.0, 400.0).unwrap()
    };
    let (fa, fd) = (f0(&alfa[0]), f0(&delta[0]));
    assert!(
        (fa - 200.0).abs() < 4.0 && (fd - 224.0).abs() < 4.5,
        "Alfa {fa} Hz, Delta {fd} Hz"
    );
    assert!(delta[0].audio.pcm.len() < alfa[0].audio.pcm.len());
}

#[tokio::test]
async fn fallback_to_piper_with_estimated_marks() {
    let ctl = SidecarCtl::default();
    ctl.fail.store(true, Ordering::SeqCst);
    let tts = service(&ctl);
    let chunks = collect(
        &tts,
        request(3, PersonaId::beta(), "Już sprawdzam. Chwileczkę."),
        CancelToken::new(),
    )
    .await
    .unwrap();
    assert!(
        chunks
            .iter()
            .all(|c| c.engine == "piper" && c.marks_kind == MarksKind::Estimated)
    );
    let ev = tts.take_events();
    assert_eq!(
        ev.iter()
            .filter(|e| matches!(e, TtsEvent::Fallback { .. }))
            .count(),
        1,
        "fallback raz na wypowiedź"
    );
    assert!(matches!(tts.health(), TtsHealth::Degraded(_)));
}

#[tokio::test]
async fn private_session_skips_cloud_voice() {
    let mut chains = v0_chains();
    let cloud = VoiceRef {
        engine: TtsEngine::Cloud {
            provider: CloudTts::ElevenLabs,
            account: "k".into(),
            voice: "alfa-v1".into(),
        },
        ..chains[0].1[0].clone()
    };
    chains[0].1.insert(0, cloud);
    let tts = TtsService::new(chains, vec![pocket(SidecarCtl::default())]).unwrap();
    let mut req = request(4, PersonaId::alfa(), "Tajne.");
    req.privacy = PrivacyTag::Private;
    let chunks = collect(&tts, req, CancelToken::new()).await.unwrap();
    assert_eq!(chunks[0].engine, "pocket");
    assert!(
        tts.take_events()
            .iter()
            .any(|e| matches!(e, TtsEvent::Fallback { reason, .. } if reason.contains("prywatna")))
    );
    assert!(
        !tts.take_events()
            .iter()
            .any(|e| matches!(e, TtsEvent::CloudSent { .. }))
    );
}

#[tokio::test]
async fn phrase_cache_serves_second_request_without_engine() {
    let dir = std::env::temp_dir().join(format!("alfa-tts-svc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctl = SidecarCtl::default();
    let tts = service(&ctl).with_cache(PhraseCache::new(dir.clone(), 50 << 20).unwrap());
    let mut req = request(5, PersonaId::gama(), "Gotowe, przekazuję Delcie.");
    req.cacheable = true;
    let first = collect(&tts, req.clone(), CancelToken::new())
        .await
        .unwrap();
    let calls = ctl.requests.load(Ordering::SeqCst);
    req.utterance = 6;
    let second = collect(&tts, req, CancelToken::new()).await.unwrap();
    assert_eq!(
        ctl.requests.load(Ordering::SeqCst),
        calls,
        "bez wywołania silnika"
    );
    assert_eq!(
        second[0].audio.pcm.len(),
        first.iter().map(|c| c.audio.pcm.len()).sum::<usize>()
    );
    assert!(tts.take_events().iter().any(|e| matches!(
        e,
        TtsEvent::Started {
            utterance: 6,
            cached: true,
            ..
        }
    )));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn stop_cancels_running_job_and_errors() {
    let tts = service(&SidecarCtl::default());
    let mut rx = tts
        .synth(
            request(7, PersonaId::alfa(), "Raz. Dwa. Trzy."),
            CancelToken::new(),
        )
        .await
        .unwrap();
    tts.stop(7); // zadanie jeszcze nie ruszyło (runtime jednowątkowy)
    assert!(matches!(rx.recv().await, Some(Err(TtsError::Cancelled))));
    assert!(tts.warm(&PersonaId::alfa()).await.is_ok());
    let none = TtsService::new(v0_chains(), vec![]).unwrap();
    assert!(none.warm(&PersonaId::alfa()).await.is_err());
    assert!(matches!(none.health(), TtsHealth::Failed(_)));
    let r = collect(
        &none,
        request(8, PersonaId::alfa(), "Hej."),
        CancelToken::new(),
    )
    .await;
    assert!(matches!(r, Err(TtsError::AllEnginesFailed(_))));
    let mut bad = v0_chains();
    // Ogniwo 0 to Supertonic (własna barwa na agentkę); 1 — Pocket, gdzie odrębność daje preset.
    bad[1].1[1].preset.pitch = 1.0;
    bad[1].1[1].preset.rate = 1.0;
    bad[1].1[1].preset.base_speaker = "pl-f1".into();
    assert!(
        TtsService::new(bad, vec![]).is_err(),
        "Beta brzmiałaby jak Alfa"
    );
}

#[tokio::test]
async fn residency_lease_and_module() {
    let residency = Arc::new(model_residency_fake::FakeResidency::new(
        model_residency_contract::Budget {
            vram_mb: 0,
            ram_mb: 4_000,
            desktop_reserve_mb: 768,
            stt_tts_exclusive: false,
        },
    ));
    let lease = TtsLease::acquire(residency.clone(), "pocket-tts-pl", 800, 300_000).unwrap();
    assert_eq!(residency.snapshot().leases[0].request.owner, "voice-tts");
    let _ = lease.id();
    drop(lease);
    assert!(residency.snapshot().leases.is_empty());
    let mut m = VoiceTtsModule::new().unwrap();
    use core_registry_contract::Module;
    let bus = core_bus_fake::FakeBus::default();
    assert_eq!(m.publish(&[TtsEvent::Stopped { utterance: 1 }]).await, 0);
    m.start(core_registry_contract::ModuleContext::new(
        m.manifest().id.clone(),
        Arc::new(bus.clone()),
    ))
    .await
    .unwrap();
    assert_eq!(m.publish(&[TtsEvent::Stopped { utterance: 1 }]).await, 1);
    assert_eq!(
        bus.recorded_of_kind(&voice_tts_contract::event_kind("voice.tts.stopped"))
            .len(),
        1
    );
    m.stop().await.unwrap();
}

/// Piper zapisujący argumenty wywołań — sprawdzamy, co naprawdę trafia do silnika.
struct RecordingPiper(Arc<std::sync::Mutex<Vec<Vec<String>>>>);

#[async_trait]
impl ProcessRunner for RecordingPiper {
    async fn run(
        &self,
        _p: &Path,
        args: &[String],
        stdin: Vec<u8>,
        _t: Duration,
    ) -> Result<Vec<u8>, String> {
        self.0.lock().unwrap().push(args.to_vec());
        Ok(tone_pcm16(
            String::from_utf8_lossy(&stdin).trim().chars().count(),
            22_050,
        ))
    }
}

fn arg(args: &[String], name: &str) -> f32 {
    let at = args.iter().position(|a| a == name).expect(name);
    args[at + 1].parse().expect("liczba")
}

/// Zgłoszenie właściciela (2026-10-09): głos sztywny i niemiły. Strojenie z Ustawień trafia do
/// Pipera (tempo natywnie, ekspresja, rytm), odrębność głosów skaluje przestrajanie, a po zdaniu
/// jest pauza.
#[tokio::test]
async fn tuning_reaches_piper_and_shapes_the_audio() {
    use voice_tts_contract::{SharedTuning, TtsTuning};
    let calls: Arc<std::sync::Mutex<Vec<Vec<String>>>> = Arc::default();
    let tuning = SharedTuning::default();
    let engine: Arc<dyn TtsBackend> = Arc::new(
        PiperEngine::new(
            "piper".into(),
            "/modele".into(),
            Arc::new(RecordingPiper(calls.clone())),
        )
        .with_tuning(tuning.clone()),
    );
    let tts = TtsService::new(v0_chains(), vec![engine])
        .unwrap()
        .with_tuning(tuning.clone());
    let say = |id: u64| request(id, PersonaId::delta(), "Już sprawdzam.");

    // Ustawienia domyślne silnika: pełne różnice głosów, bez pauzy. Delta mówi szybciej (preset
    // 1.08), a tempo ustawia sam Piper — długość zdania skraca `--length_scale`, nie rozciąganie.
    let plain = collect(&tts, say(1), CancelToken::new()).await.unwrap();
    let first = calls.lock().unwrap().last().cloned().unwrap();
    assert!((arg(&first, "--length_scale") - 1.0 / 1.08).abs() < 0.01);
    assert!((arg(&first, "--noise_scale") - 0.667).abs() < 0.01);
    assert!((arg(&first, "--noise_w") - 0.8).abs() < 0.01);

    // Strojenie użytkownika: wolniej, żywiej, jeden głos, pauza 300 ms.
    tuning.set(TtsTuning {
        speed: 0.8,
        expressiveness: 0.9,
        rhythm: 1.0,
        pause_ms: 300,
        character: 0.0,
        volume: 1.0,
        ..TtsTuning::default()
    });
    let tuned = collect(&tts, say(2), CancelToken::new()).await.unwrap();
    let second = calls.lock().unwrap().last().cloned().unwrap();
    assert!((arg(&second, "--length_scale") - 1.25).abs() < 0.01);
    assert!((arg(&second, "--noise_scale") - 0.9).abs() < 0.01);
    assert!((arg(&second, "--noise_w") - 1.0).abs() < 0.01);

    // Jeden głos = bez przestrajania: wysokość tonu wraca do bazowej (atrapa gra 200 Hz).
    let f0 = |c: &voice_tts_contract::TtsChunk| -> f32 {
        estimate_f0(&c.audio.pcm[1_000..5_800], 24_000, 80.0, 400.0).unwrap()
    };
    let (delta_pitch, base_pitch) = (f0(&plain[0]), f0(&tuned[0]));
    assert!((base_pitch - 200.0).abs() < 4.0, "bazowy {base_pitch} Hz");
    assert!(delta_pitch > base_pitch + 15.0, "Delta {delta_pitch} Hz");

    // Pauza: zdanie kończy się 300 ms ciszy.
    let pcm = &tuned[0].audio.pcm;
    let pause = 24_000 * 300 / 1000;
    assert!(pcm.len() > pause);
    assert!(pcm[pcm.len() - pause..].iter().all(|s| *s == 0.0));
    assert!(plain[0].audio.pcm.iter().rev().take(100).any(|s| *s != 0.0));
}
