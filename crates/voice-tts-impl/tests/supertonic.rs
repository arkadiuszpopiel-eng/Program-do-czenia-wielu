//! Supertonic 3 w łańcuchu głosów: sidecar dostaje barwę agentki i tempo, a silnik
//! niezainstalowany jest pomijany bez zdarzenia awarii.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use personas_contract::PersonaId;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use voice_tts_contract::contract_tests::{collect, request};
use voice_tts_contract::{CancelToken, Tts, TtsEvent, TtsHealth, v0_chains};
use voice_tts_impl::TtsService;
use voice_tts_impl::engines::{PiperEngine, ProcessRunner, TtsBackend};
use voice_tts_impl::sidecar::{PocketSidecar, SUPERTONIC_BASE_RATE};

fn tone(samples: usize) -> Vec<u8> {
    (0..samples)
        .flat_map(|i| (((i as f32 * 0.05).sin() * 8000.0) as i16).to_le_bytes())
        .collect()
}

/// Udawany `alfa-supertonic`: zapisuje żądania, odpowiada dźwiękiem 44,1 kHz i `done`.
fn supertonic(seen: Arc<Mutex<Vec<serde_json::Value>>>) -> Arc<dyn TtsBackend> {
    let (client, server) = tokio::io::duplex(1 << 20);
    let (sr, mut sw) = tokio::io::split(server);
    tokio::spawn(async move {
        let mut lines = BufReader::new(sr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let Ok(req) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let id = req["id"].as_u64().unwrap_or(0);
            seen.lock().unwrap_or_else(|e| e.into_inner()).push(req);
            let b64 = base64::engine::general_purpose::STANDARD.encode(tone(22_050));
            let out = format!(
                "{{\"type\":\"audio\",\"id\":{id},\"pcm16\":\"{b64}\",\"sample_rate\":44100}}\n{{\"type\":\"done\",\"id\":{id}}}\n"
            );
            if sw.write_all(out.as_bytes()).await.is_err() {
                break;
            }
        }
    });
    let (cr, cw) = tokio::io::split(client);
    Arc::new(PocketSidecar::with_channel(Box::new(BufReader::new(cr)), Box::new(cw)).supertonic())
}

struct FakePiper;

#[async_trait]
impl ProcessRunner for FakePiper {
    async fn run(
        &self,
        _p: &Path,
        _a: &[String],
        _i: Vec<u8>,
        _t: Duration,
    ) -> Result<Vec<u8>, String> {
        Ok(tone(11_025))
    }
}

async fn engines_used(
    tts: &TtsService,
    utterance: u64,
    persona: PersonaId,
    text: &str,
) -> Vec<String> {
    collect(tts, request(utterance, persona, text), CancelToken::new())
        .await
        .expect("synteza")
        .into_iter()
        .map(|c| c.engine)
        .collect()
}

#[tokio::test]
async fn supertonic_speaks_first_with_agent_timbre_and_native_tempo() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let backend = supertonic(Arc::clone(&seen));
    assert_eq!(backend.engine().name(), "supertonic");
    assert!(backend.native_tempo(), "tempo ustawia silnik");
    let tts = TtsService::new(v0_chains(), vec![backend]).expect("serwis");
    let used = engines_used(&tts, 1, PersonaId::delta(), "Dzień dobry.").await;
    assert!(!used.is_empty() && used.iter().all(|e| e == "supertonic"));
    let seen = seen.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(seen[0]["voice"], "F5", "barwa Delty");
    let rate = seen[0]["rate"].as_f64().expect("tempo w żądaniu");
    // Tempo agentki × tempo naturalne modelu; Delta mówi żwawiej niż 1,05.
    assert!(
        rate > f64::from(SUPERTONIC_BASE_RATE) && rate <= 2.0,
        "tempo {rate}"
    );
    assert!(
        tts.take_events()
            .iter()
            .all(|e| !matches!(e, TtsEvent::Fallback { .. })),
        "bez przejścia na zapas"
    );
}

#[tokio::test]
async fn uninstalled_engine_is_skipped_without_fallback_event() {
    let piper: Arc<dyn TtsBackend> = Arc::new(PiperEngine::new(
        "piper".into(),
        "/modele".into(),
        Arc::new(FakePiper),
    ));
    let tts = TtsService::new(v0_chains(), vec![piper]).expect("serwis");
    let used = engines_used(&tts, 2, PersonaId::alfa(), "Sprawdzam.").await;
    assert!(!used.is_empty() && used.iter().all(|e| e == "piper"));
    assert!(
        tts.take_events()
            .iter()
            .all(|e| !matches!(e, TtsEvent::Fallback { .. })),
        "brak Supertonica i Pocketa to nie awaria"
    );
    assert!(matches!(tts.health(), TtsHealth::Ready));
}
