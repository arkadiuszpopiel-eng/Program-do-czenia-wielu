//! E2E na atrapach: pełna tura (mowa → odpowiedź słyszalna), czas do pierwszego audio na
//! budżecie profilu A (100 losowych scenariuszy), awaria STT GPU → CPU bez utraty wypowiedzi,
//! test kontraktowy, rezydencja modeli, zdarzenia bez treści audio.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::{Opts, Timeline, World, percentile};
use device_profile_contract::Backend;
use model_residency_contract::{LeaseRequest, ModelRole, Placement, Priority, Residency};
use voice_dialog_contract::{Command, DialogPhase};
use voice_pipeline_contract::{
    EVENT_DEGRADED, EVENT_LATENCY, EVENT_PILL, EVENT_TRANSCRIPT, PipelineInput, VoicePipeline,
    contract_tests,
};

#[tokio::test]
async fn full_turn_speech_to_audible_reply() {
    let mut w = World::new(Opts::default());
    let mut t = Timeline::new(8_000, 1);
    t.speech(500, 1_400, 11);
    w.mic(&t);
    w.stt.script("Jaka jutro będzie pogoda?");
    w.answer(&["Jutro będzie słonecznie."]);
    w.conversation_mode().await;
    w.run_until(6_000).await;
    let s = w.p.status();
    assert_eq!(s.turns, 1, "{s:?}");
    assert_eq!(s.phase, DialogPhase::Listening, "{s:?}");
    let first = w
        .first_audio_after(1_900, 6_000)
        .expect("odpowiedź słyszalna");
    let ttfa = first - 1_900;
    eprintln!(
        "czas do pierwszego audio (atrapy bez opóźnień): {ttfa} ms; {:?}",
        s.latency
    );
    assert!(ttfa < 1_000, "{ttfa}");
    // Model dostał turę użytkownika, a historia zamknęła turę asystentki (wysłuchana).
    let req = &w.provider.requests()[0];
    assert_eq!(
        req.messages.last().unwrap().visible_text(),
        "Jaka jutro będzie pogoda?"
    );
    assert!(req.system.as_deref().unwrap().contains("Alfa"));
    let history = w.reply.history();
    assert_eq!(history.len(), 2);
    assert!(history[1].interruption.is_none());
    assert!(history[1].visible_text().contains("słonecznie"));
    assert!(!w.events(EVENT_TRANSCRIPT).is_empty());
    assert!(!w.events(EVENT_PILL).is_empty());
    assert_eq!(w.events(EVENT_LATENCY).len(), 1);
    assert!(s.latency.time_to_first_audio_ms().is_some());
    assert!(
        w.trace_commands()
            .iter()
            .any(|(_, c)| matches!(c, Command::SubmitTurn { .. }))
    );
    w.check_logs();
}

/// Losowy generator testów (xorshift) — deterministyczny.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

/// Jeden losowy scenariusz: składowe opóźnień z budżetu profilu A (VOICE.md §4, lokalnie):
/// koniec tury 250–500 ms, final STT 150–300, TTFT 200–600, TTFB TTS 200–500.
async fn latency_scenario(seed: u64) -> (u64, u64) {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let opts = Opts {
        stt_latency_ms: u32::try_from(rng.range(150, 300)).unwrap(),
        tts_ttfb_ms: u32::try_from(rng.range(200, 500)).unwrap(),
        ttft_ms: rng.range(200, 600),
        ..Opts::default()
    };
    let eot = rng.range(250, 500);
    let speech_ms = rng.range(700, 2_200);
    let mut w = World::new(opts);
    w.turn.0.lock().unwrap().push_delay(eot);
    let mut t = Timeline::new(speech_ms + 4_500, seed);
    t.speech(300, speech_ms, seed + 7);
    w.mic(&t);
    w.stt.script("Jakie mam dziś spotkania w kalendarzu?");
    w.answer(&["Masz dziś dwa spotkania.", "Pierwsze o dziesiątej."]);
    w.conversation_mode().await;
    let end = speech_ms + 4_300;
    while w.now() < end
        && w.p
            .status()
            .latency
            .first_audio_ms
            .is_none_or(|t| w.now() < t + 50)
    {
        w.tick().await;
    }
    let l = w.p.status().latency;
    let Some(ttfa) = l.time_to_first_audio_ms() else {
        let tail: Vec<_> = w.trace_commands().into_iter().rev().take(30).collect();
        panic!(
            "seed {seed}: brak pierwszego audio; {:?}; {tail:#?}",
            w.p.status()
        );
    };
    // Pomiar niezależny („loopback”): koniec mowy wg VAD → pierwsza próbka nagranego wyjścia.
    let speech_end = l.speech_end_ms.unwrap();
    let heard = w.first_audio_after(speech_end, w.now()).unwrap() - speech_end;
    w.check_logs();
    (ttfa, heard)
}

#[tokio::test]
async fn time_to_first_audio_profile_a_budget_100_scenarios() {
    let mut ttfa = Vec::new();
    let mut loopback = Vec::new();
    for seed in 1..=100 {
        let (a, b) = latency_scenario(seed).await;
        assert!(
            a.abs_diff(b) <= 20,
            "znaczniki potoku vs loopback: {a} vs {b}"
        );
        ttfa.push(a);
        loopback.push(b);
    }
    let (p50, p95) = (percentile(&ttfa, 50.0), percentile(&ttfa, 95.0));
    let (l50, l95) = (percentile(&loopback, 50.0), percentile(&loopback, 95.0));
    eprintln!(
        "czas do pierwszego audio, profil A (100 scenariuszy, zegar wirtualny): p50 {p50} ms, p95 {p95} ms \
         (loopback p50 {l50} ms, p95 {l95} ms); min {} ms, max {} ms; budżet p50 ≤ 2000, p95 ≤ 3000",
        ttfa.iter().min().unwrap(),
        ttfa.iter().max().unwrap()
    );
    assert!(p50 <= 2_000 && p95 <= 3_000, "p50 {p50}, p95 {p95}");
}

/// (h) Awaria STT na GPU: atrapa przechodzi na CPU w trakcie finalu — wypowiedź nie ginie;
/// błąd sidecara bez wewnętrznego fallbacku → potok ponawia rozpoznanie z bufora wypowiedzi.
#[tokio::test]
async fn stt_gpu_failure_falls_back_without_losing_the_utterance() {
    let mut w = World::new(Opts::default());
    let mut t = Timeline::new(9_000, 3);
    t.speech(300, 1_200, 4);
    t.speech(4_300, 1_200, 5);
    w.mic(&t);
    w.stt.script("Otwórz raport kwartalny.");
    w.stt.script("A teraz pokaż wykres.");
    w.stt.crash_next();
    w.answer(&["Otwieram raport."]);
    w.answer(&["Pokazuję wykres."]);
    w.conversation_mode().await;
    w.run_until(4_000).await;
    assert_eq!(w.stt.backend(), Backend::Cpu);
    let fallback = w.events(voice_stt_contract::EVENT_BACKEND_FALLBACK);
    assert_eq!(fallback.len(), 1, "zdarzenie voice.stt.backend.fallback");
    assert_eq!(
        w.provider.requests()[0]
            .messages
            .last()
            .unwrap()
            .visible_text(),
        "Otwórz raport kwartalny."
    );
    // Drugi raz: sidecar pada bez fallbacku — potok ponawia z buforem.
    w.slow_stt.fail_next_end.store(true, Ordering::SeqCst);
    w.run_until(8_500).await;
    let reqs = w.provider.requests();
    assert_eq!(reqs.len(), 2, "druga tura dotarła do modelu mimo błędu STT");
    assert_eq!(
        reqs[1].messages.last().unwrap().visible_text(),
        "A teraz pokaż wykres."
    );
    let degraded: Vec<String> = w
        .events(EVENT_DEGRADED)
        .iter()
        .map(|e| e.payload["reason"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(
        degraded.iter().any(|r| r.contains("ponawiam")),
        "{degraded:?}"
    );
    assert!(
        w.first_audio_after(6_000, 8_500).is_some(),
        "odpowiedź słyszalna"
    );
    w.check_logs();
}

/// Potok przesuwający zegar urządzenia przy każdym kroku (dla wspólnego testu kontraktowego).
struct Ticking(World);

#[async_trait::async_trait]
impl VoicePipeline for Ticking {
    fn input(&mut self, input: PipelineInput) {
        self.0.p.input(input);
    }
    async fn step(&mut self) -> voice_pipeline_contract::StepReport {
        self.0.audio.advance(std::time::Duration::from_millis(10));
        self.0.p.step().await
    }
    fn status(&self) -> voice_pipeline_contract::PipelineStatus {
        self.0.p.status()
    }
}

#[tokio::test]
async fn contract_suite() {
    let mut w = Ticking(World::new(Opts::default()));
    contract_tests::run_all(&mut w).await;
    w.0.check_invariants();
}

/// Rezydencja: w trakcie rozmowy dzierżawy modeli głosu są przypięte, po wyjściu — odpięte.
#[tokio::test]
async fn voice_models_are_pinned_in_residency_while_talking() {
    let residency = model_residency_fake::FakeResidency::baseline();
    let req = |owner: &str, role| LeaseRequest {
        owner: owner.into(),
        model: format!("{owner}-model"),
        role,
        priority: Priority::VoiceRt,
        placement: Placement::CpuOnly,
        vram_mb: 0,
        ram_mb: 0,
        cpu_ram_mb: 300,
        idle_unload_ms: 60_000,
    };
    let stt = residency
        .acquire(req("voice-stt", ModelRole::Stt))
        .unwrap()
        .lease
        .id;
    let tts = residency
        .acquire(req("voice-tts", ModelRole::Tts))
        .unwrap()
        .lease
        .id;
    let other = residency
        .acquire(req("search", ModelRole::Embedder))
        .unwrap()
        .lease
        .id;
    let mut w = World::with_residency(Opts::default(), Arc::new(residency.clone()));
    w.mic(&Timeline::new(3_000, 9));
    w.conversation_mode().await;
    w.run_until(200).await;
    assert_eq!(w.p.pinned_models(), 2);
    assert!(residency.lease(stt).unwrap().in_use && residency.lease(tts).unwrap().in_use);
    assert!(
        !residency.lease(other).unwrap().in_use,
        "tylko modele głosu"
    );
    w.p.input(PipelineInput::Deactivate);
    w.run_until(400).await;
    assert_eq!(w.p.pinned_models(), 0);
    assert!(!residency.lease(stt).unwrap().in_use);
}

/// Zdarzenia `voice.*` niosą metadane i tekst — nigdy próbki audio.
#[tokio::test]
async fn bus_events_carry_no_audio() {
    let mut w = World::new(Opts::default());
    let mut t = Timeline::new(6_000, 2);
    t.speech(400, 1_000, 3);
    w.mic(&t);
    w.stt.script("Powiedz coś miłego.");
    w.answer(&["Masz świetny dzień."]);
    w.conversation_mode().await;
    w.run_until(5_000).await;
    let all = w.bus.recorded();
    assert!(all.len() > 50, "{}", all.len());
    for e in &all {
        assert!(
            e.kind.as_str().starts_with("voice.") || e.kind.as_str().starts_with("scheduler."),
            "{}",
            e.kind.as_str()
        );
        let text = e.payload.to_string();
        assert!(text.len() < 4_096, "{}: {} B", e.kind.as_str(), text.len());
        for key in ["\"pcm\"", "\"samples\"", "\"audio\""] {
            assert!(!text.contains(key), "{}: {text}", e.kind.as_str());
        }
    }
    for name in [
        "voice.wake.mic_state",
        "voice.pipeline.pill",
        "voice.pipeline.transcript",
        "voice.dialog.state_changed",
        "voice.vad.speech_start",
        "voice.stt.final",
        "voice.tts.started",
        "voice.audio.playback.started",
    ] {
        assert!(!w.events(name).is_empty(), "brak zdarzenia {name}");
    }
    assert_eq!(w.p.dropped_events(), 0);
}
