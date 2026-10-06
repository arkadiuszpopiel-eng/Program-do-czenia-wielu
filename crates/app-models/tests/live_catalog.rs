//! Silniki na żywo (CPU) — próba generalna przed testem na laptopie właściciela (fala 6;
//! `.github/workflows/rehearsal.yml`, job „Silniki na żywo”). Uruchamiany tylko ręcznie albo w tym
//! jobie: `ALFA_LIVE_CATALOG=1 cargo test -p app-models --test live_catalog -- --ignored --nocapture`.
//!
//! Przez **prawdziwy** `ModelsApp` (katalog `builtin()`, HTTPS, zgoda TOFU jak kliknięcie w UI)
//! pobiera i rozpakowuje: `llama-server` (CPU) + modele GGUF z katalogu (oficjalne Bielik v3.0 Q8_0:
//! 1.5B ~1,6 GB i domyślny 4.5B ~4,8 GB — ten, który pobierze właściciel; `ALFA_LIVE_LLM=<id>` zawęża
//! do jednego), `whisper-server` (CPU) + najmniejszy model whisper, `piper` + głos `pl_PL`; dodatkowo — tylko pobranie
//! i rozpakowanie (bez GPU na runnerze) — `llama-server` Vulkan i CUDA (+ `cudart`) oraz
//! `whisper-server` CUDA, jeśli są w katalogu. Silniki uruchamia kodem aplikacji:
//! `app_modules::route::local` (`llama-server`, ta sama kompozycja co `app-core`),
//! `app_modules::tts::engines` (Piper) i `app_modules::stt::whisper` (to samo co `app-voice`).
//! Sprawdza: generację ≥ 16 tokenów po polsku (każdy model; z narzędziem, gdy `tools = true`) i wiersze
//! logu `llama-server` z architekturą i KV cache; „Dzień dobry, jestem Alfa.” → Piper → WAV →
//! whisper → WER. Brak pozycji nie przerywa testu — dalsze etapy idą z tym, co się zainstalowało. Raport JSON (`ALFA_LIVE_REPORT`, domyślnie `<katalog>/live-report.json`):
//! adres, rozmiar i **SHA-256** każdego pobranego pliku, układ drzewa, wersje i flagi CLI, czasy
//! (pobieranie, start sidecara, tok/s, RTF). Raport służy człowiekowi do przypięcia hashy —
//! test niczego nie przypina w katalogu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod live;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use app_api::AppPaths;
use app_api::dto::ModelItemKind;
use app_models::catalog::ItemSpec;
use app_models::store::Store;
use app_models::{ModelsApp, ModelsDeps, ModelsOptions};
use device_profile_contract::DeviceProfile;
use device_profile_fake::FakeDeviceProfile;
use futures_util::StreamExt;
use live::setup::{llms, mirrored, root, runner_profile, smallest};
use live::{Report, ms};
use model_residency_contract::Residency;
use providers_contract::{
    CancellationToken, ChatRequest, Message, ModelProvider, PrivacyTag, ProviderEvent, ToolSpec,
};
use providers_local_impl::LocalEvent;
use serde_json::json;
use voice_audio_contract::wav::{WavEncoding, encode_wav};
use voice_audio_contract::{AudioFormat, Frame, MediaTime, Resampler};
use voice_stt_contract::{Stt, SttEvent, UtteranceId};
use voice_stt_impl::{ProcessLauncher, WhisperStt};
use voice_tts_contract::{CancelToken, SpeechStyle, TtsRequest};

/// Zdanie testu głosu.
const PHRASE: &str = "Dzień dobry, jestem Alfa.";
/// Próg WER dla małego modelu whisper na mowie syntetycznej (4 słowa: dopuszczalne 2 błędy).
const MAX_WER: f64 = 0.5;
/// Głos Piper sprawdzany w teście (polski, z katalogu `app-models`).
const PIPER_VOICE: &str = "piper-pl_PL-gosia-medium";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "pobiera ~4 GB z GitHub/Hugging Face i uruchamia silniki; ALFA_LIVE_CATALOG=1"]
async fn live_catalog_cpu_engines() {
    if std::env::var_os("ALFA_LIVE_CATALOG").is_none() {
        eprintln!("pominięte: ustaw ALFA_LIVE_CATALOG=1 (próba generalna, rehearsal.yml)");
        return;
    }
    let root = root();
    let paths = AppPaths::under(&root);
    paths.ensure().unwrap();
    let report_path = std::env::var_os("ALFA_LIVE_REPORT")
        .map_or_else(|| root.join("live-report.json"), PathBuf::from);
    let mut report = Report::new(report_path.clone());
    let (catalog, mirror) = mirrored(app_models::builtin());
    let llms = llms(&catalog);
    let stt_model = smallest(&catalog, ModelItemKind::Stt);
    let voice = [
        "sidecar-whisper-cpu",
        stt_model.id.as_str(),
        "sidecar-piper",
        PIPER_VOICE,
    ];
    let mut core = vec!["sidecar-llama-cpu"];
    core.extend(llms.iter().map(|l| l.id.as_str()));
    core.extend(voice);
    let layout_only = [
        "sidecar-llama-vulkan",
        "sidecar-llama-cuda",
        "sidecar-whisper-cuda",
    ];
    let wanted: Vec<ItemSpec> = core
        .iter()
        .chain(&layout_only)
        .filter_map(|id| catalog.iter().find(|i| i.id == *id).cloned())
        .collect();
    report.set(
        "selected",
        json!(wanted.iter().map(|i| &i.id).collect::<Vec<_>>()),
    );
    let app = ModelsApp::open(ModelsDeps {
        paths: paths.clone(),
        catalog: catalog.clone(),
        events: None,
        embed: None,
        options: ModelsOptions {
            parallel: 3,
            loopback_http: mirror,
            startup_delay: None,
            ..ModelsOptions::default()
        },
    });
    let store = Store::new(paths.clone());
    let started = Instant::now();
    for spec in &wanted {
        app.download(&spec.id).await.unwrap();
    }
    let mut installed = Vec::new();
    for spec in &wanted {
        if live::install(&app, &store, spec, started, &mut report).await {
            installed.push(spec.id.clone());
        }
    }
    let has = |id: &str| installed.iter().any(|i| i == id);
    for id in core.iter().filter(|id| !has(id)) {
        report.problem(format!("nie zainstalowano: {id}"));
    }
    live::probe::probe_programs(&paths, &mut report).await;
    if has("sidecar-llama-cpu") {
        live::serverlog::install();
        for llm in llms.iter().filter(|l| has(&l.id)) {
            llm_generation(&paths, &llm.id, &mut report).await;
        }
    }
    if voice.iter().all(|id| has(id)) {
        voice_round_trip(&paths, &root, &mut report).await;
    }
    let problems = report.problems();
    assert!(
        problems.is_empty(),
        "problemy ({}): {problems:#?}",
        report_path.display()
    );
}

/// Generacja po polsku przez dostawcę lokalnego złożonego jak w aplikacji.
async fn llm_generation(paths: &AppPaths, model: &str, report: &mut Report) {
    let device: Arc<dyn DeviceProfile> = Arc::new(FakeDeviceProfile::new(runner_profile()));
    let residency = app_modules::route::local::residency(&device).unwrap();
    let manager = residency.manager() as Arc<dyn Residency>;
    let module = app_modules::route::local::provider_module(paths, &device, Some(manager)).unwrap();
    let provider = module.provider();
    let (sink, mut local_events) = tokio::sync::mpsc::unbounded_channel();
    provider.sidecar().set_event_sink(Some(sink));
    let mut req = ChatRequest::new(
        model,
        vec![Message::user_text(
            "Opisz po polsku w trzech zdaniach, czym zajmuje się asystentka głosowa.",
        )],
    );
    req.params.max_tokens = Some(96);
    let t0 = Instant::now();
    let mut stream = provider.stream(req, CancellationToken::new());
    let (mut first, mut text, mut tokens, mut error) = (None, String::new(), 0u64, None);
    while let Some(event) = stream.next().await {
        match event {
            ProviderEvent::TextDelta { text: t, .. } => {
                first.get_or_insert_with(|| t0.elapsed());
                text.push_str(&t);
            }
            ProviderEvent::Usage(u) => tokens = u.output_tokens,
            ProviderEvent::Error(e) => error = Some(e.to_string()),
            _ => {}
        }
    }
    let total = t0.elapsed();
    let first = first.unwrap_or(total);
    let gen_s = (total - first).as_secs_f64().max(0.001);
    // Bez `usage` w strumieniu (inna wersja serwera) — szacunek ze słów (≈ 1,5 tokenu na słowo PL).
    let estimated = tokens == 0;
    if estimated {
        tokens = live::words(&text).len() as u64 * 3 / 2;
    }
    let plan = provider.sidecar().running_plan().await;
    let (mut startup_ms, mut sidecar_events) = (None, Vec::new());
    while let Ok(event) = local_events.try_recv() {
        match event {
            LocalEvent::Loaded { startup_ms: t, .. } => startup_ms = Some(t),
            other => sidecar_events.push(format!("{other:?}")),
        }
    }
    let pl = text.chars().any(|c| "ąćęłńóśźż".contains(c));
    let mut entry = json!({
            "model": model,
            "server": plan.as_ref().map(|(_, p, _)| p.program.display().to_string()),
            "backend": plan.as_ref().map(|(_, p, _)| p.backend.as_str()),
            "threads": plan.as_ref().map(|(_, p, _)| p.threads),
            "sidecar_startup_ms": startup_ms,
            "sidecar_events": sidecar_events,
            "first_token_ms": ms(first),
            "total_ms": ms(total),
            "output_tokens": tokens,
            "tokens_estimated": estimated,
            "tok_per_s": tokens as f64 / gen_s,
            "polish_letters": pl,
            "text": text,
            "error": error,
    });
    if error.is_some() || tokens < 16 || !pl {
        report.problem(format!(
            "LLM {model}: {tokens} tokenów, polskie litery: {pl}, błąd: {error:?}"
        ));
    }
    entry["tools"] = if provider.entry(model).is_some_and(|e| e.tools) {
        tool_request(provider.as_ref(), model, report).await
    } else {
        // Lekki model bez narzędzi (`tools = false` w manifeście) — serwer bez `--jinja`.
        json!({ "skipped": "model bez narzędzi" })
    };
    provider.sidecar().stop("koniec testu").await;
    entry["server_log"] = json!(live::serverlog::drain());
    report.push("llm", entry);
}

/// Żądanie z narzędziem (jak tura agentki): szablon czatu modelu z `--jinja` musi przyjąć
/// `tools` — błąd serwera to problem; to, czy mały model wywoła narzędzie, trafia tylko do raportu.
async fn tool_request(
    provider: &dyn ModelProvider,
    model: &str,
    report: &mut Report,
) -> serde_json::Value {
    let mut req = ChatRequest::new(
        model,
        vec![Message::user_text(
            "Jaka jest teraz pogoda w Krakowie? Sprawdź narzędziem.",
        )],
    );
    req.tools = vec![ToolSpec {
        name: "pogoda".into(),
        description: "Zwraca bieżącą pogodę dla miasta.".into(),
        input_schema: json!({"type": "object", "properties": {"miasto": {"type": "string"}},
                             "required": ["miasto"]}),
        strict: false,
    }];
    req.params.max_tokens = Some(128);
    let t0 = Instant::now();
    let events: Vec<ProviderEvent> = provider
        .stream(req, CancellationToken::new())
        .collect()
        .await;
    let called = events
        .iter()
        .any(|e| matches!(e, ProviderEvent::ToolCallStart { .. }));
    let error = events.iter().find_map(|e| match e {
        ProviderEvent::Error(err) => Some(err.to_string()),
        _ => None,
    });
    if let Some(e) = &error {
        report.problem(format!("LLM {model} z narzędziami: {e}"));
    }
    json!({ "tool_called": called, "total_ms": ms(t0.elapsed()), "error": error })
}

/// Zdarzenia STT w skrócie (fallback z powodem — na runnerze bez GPU wersja CUDA nie startuje).
fn stt_events(stt: &WhisperStt) -> Vec<String> {
    stt.take_events()
        .iter()
        .map(|e| match e {
            SttEvent::BackendFallback { from, to, reason } => {
                format!("fallback {from:?} → {to:?}: {reason}")
            }
            SttEvent::ModelLoaded { backend, .. } => format!("model załadowany ({backend:?})"),
            other => other.name().to_owned(),
        })
        .collect()
}

/// TTS Piper (`app_modules::tts::engines`) → WAV → STT `whisper-server` → porównanie tekstu.
async fn voice_round_trip(paths: &AppPaths, root: &Path, report: &mut Report) {
    let tts = app_modules::tts::engines(paths);
    let request = TtsRequest {
        utterance: 1,
        persona: personas_contract::PersonaId::alfa(),
        text: PHRASE.into(),
        style: SpeechStyle::default(),
        cacheable: false,
        privacy: PrivacyTag::Normal,
    };
    let t0 = Instant::now();
    let mut rx = tts.synth(request, CancelToken::new()).await.unwrap();
    let (mut pcm, mut rate, mut ttfb, mut engine) = (Vec::new(), 0, None, String::new());
    while let Some(chunk) = rx.recv().await {
        match chunk {
            Ok(c) => {
                ttfb.get_or_insert_with(|| t0.elapsed());
                rate = c.audio.format.sample_rate;
                engine = c.engine.clone();
                pcm.extend_from_slice(&c.audio.pcm);
            }
            Err(e) => report.problem(format!("TTS: {e}")),
        }
    }
    let synth = t0.elapsed();
    let audio_ms = if rate > 0 {
        pcm.len() as u64 * 1000 / u64::from(rate)
    } else {
        0
    };
    let wav = root.join("tts-alfa.wav");
    std::fs::write(
        &wav,
        encode_wav(&pcm, AudioFormat::mono(rate.max(1)), WavEncoding::Pcm16),
    )
    .unwrap();
    report.set(
        "tts",
        json!({ "engine": engine, "rate": rate, "ttfb_ms": ttfb.map(ms), "synth_ms": ms(synth),
                "audio_ms": audio_ms, "rtf": ms(synth) as f64 / audio_ms.max(1) as f64,
                "wav": wav.display().to_string() }),
    );
    if pcm.is_empty() {
        report.problem("TTS: brak audio");
        return;
    }
    // Ta sama kompozycja co `app-voice`: wersja CUDA ma pierwszeństwo, gdy zainstalowana — na
    // runnerze bez sterownika NVIDIA jej start się nie uda i rozpoznanie przejdzie na CPU.
    let (config, preferred) = app_modules::stt::whisper(paths).expect("model whisper");
    let model = config.model_path.clone();
    let threads = config.threads;
    let stt = WhisperStt::new(config, Arc::new(ProcessLauncher), preferred).unwrap();
    // Jak w potoku: przed i po mowie cisza tła (szum −80 dB) — detektor energii bramki STT
    // uczy się poziomu szumu, zanim zacznie się mowa.
    let hush = |n: usize| {
        (0..n)
            .map(|i| (i as f32 * 0.37).sin() * 1e-4)
            .collect::<Vec<f32>>()
    };
    let mut speech = hush(8_000);
    speech.extend(Resampler::convert(rate, 16_000, &pcm));
    speech.extend(hush(4_800));
    let id = UtteranceId(1);
    let t1 = Instant::now();
    stt.start_utterance(id).await.unwrap();
    let mut partials = 0;
    for (i, c) in speech.chunks(160).enumerate() {
        let frame = Frame::mono(c.to_vec(), 16_000, MediaTime::from_ms(10 * i as u64));
        match stt.push(id, &frame).await {
            Ok(Some(_)) => partials += 1,
            Ok(None) => {}
            Err(e) => report.problem(format!("STT (partial): {e}")),
        }
    }
    let result = stt.end_utterance(id).await;
    let total = t1.elapsed();
    let text = result.as_ref().map(|t| t.text.clone()).unwrap_or_default();
    let wer = live::wer(PHRASE, &text);
    report.set(
        "stt",
        json!({ "model": model.display().to_string(), "preferred": format!("{preferred:?}"),
                "backend": result.as_ref().ok().and_then(|t| t.backend).map(|b| format!("{b:?}")),
                "threads": threads, "expected": PHRASE, "text": text,
                "lang": result.as_ref().map(|t| t.lang.clone()).ok(),
                "wer": wer, "similarity": live::similarity(PHRASE, &text), "partials": partials,
                "total_ms": ms(total), "audio_ms": audio_ms,
                "rtf": ms(total) as f64 / audio_ms.max(1) as f64,
                "error": result.as_ref().err().map(ToString::to_string),
                "events": stt_events(&stt) }),
    );
    if wer > MAX_WER {
        report.problem(format!("STT: WER {wer:.2} > {MAX_WER} („{text}”)"));
    }
}
