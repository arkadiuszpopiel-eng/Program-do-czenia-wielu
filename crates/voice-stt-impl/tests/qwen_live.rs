//! Próba na prawdziwym modelu Qwen3-ASR (domyślnie pomijana — wymaga plików spoza repozytorium):
//! `ALFA_QWEN_SERVER`, `ALFA_QWEN_MODEL`, `ALFA_QWEN_MMPROJ`, `ALFA_QWEN_WAV` (WAV PCM16 16 kHz mono)
//! i opcjonalnie `ALFA_QWEN_EXPECT` (fragment oczekiwanego tekstu), `ALFA_QWEN_CPU=1` (bez karty).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write as _;
use std::sync::Arc;

use voice_stt_contract::{Health, Stt, UtteranceId};
use voice_stt_impl::ProcessLauncher;
use voice_stt_impl::qwen::{QwenConfig, QwenStt};

fn samples(path: &str) -> Vec<f32> {
    let bytes = std::fs::read(path).expect("plik WAV");
    let data = bytes
        .windows(4)
        .position(|w| w == b"data")
        .expect("sekcja data")
        + 8;
    bytes[data..]
        .chunks_exact(2)
        .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32_768.0)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_model_transcribes_polish_speech() {
    let (Ok(server), Ok(model), Ok(mmproj), Ok(wav)) = (
        std::env::var("ALFA_QWEN_SERVER"),
        std::env::var("ALFA_QWEN_MODEL"),
        std::env::var("ALFA_QWEN_MMPROJ"),
        std::env::var("ALFA_QWEN_WAV"),
    ) else {
        let _ = writeln!(std::io::stderr(), "pominięte: brak zmiennych ALFA_QWEN_*");
        return;
    };
    let stt = QwenStt::new(
        // `ALFA_QWEN_CPU=1` — pomiar na samym procesorze (przypadek zajętej karty).
        QwenConfig::new(
            server.into(),
            model.into(),
            mmproj.into(),
            std::env::var("ALFA_QWEN_CPU").is_err(),
        ),
        Arc::new(ProcessLauncher),
    )
    .unwrap();
    assert_eq!(stt.health(), Health::Stopped);
    let pcm = samples(&wav);
    let started = std::time::Instant::now();
    let first = stt.transcribe(UtteranceId(1), &pcm, true).await.unwrap();
    let cold = started.elapsed();
    let started = std::time::Instant::now();
    let second = stt.transcribe(UtteranceId(2), &pcm, true).await.unwrap();
    let warm = started.elapsed();
    let report = format!(
        "zimny start {cold:?}, ciepłe rozpoznanie {warm:?}, język „{}”, tekst: {}",
        second.lang, second.text
    );
    let _ = writeln!(std::io::stderr(), "{report}");
    assert_eq!(first.lang, "pl");
    assert!(!second.text.is_empty());
    if let Ok(expect) = std::env::var("ALFA_QWEN_EXPECT") {
        assert!(second.text.contains(&expect), "{}", second.text);
    }
    assert!(matches!(stt.health(), Health::Ready(_)));
    assert!(!stt.unload_if_idle().await, "świeżo użyty — nie zwalniamy");
}
