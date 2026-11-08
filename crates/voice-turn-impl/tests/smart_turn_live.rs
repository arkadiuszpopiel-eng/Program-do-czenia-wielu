//! Próba na prawdziwym modelu Smart Turn przez proces `alfa-smart-turn` (pomijana bez zmiennych
//! `ALFA_SMART_TURN_EXE`, `ALFA_SMART_TURN_MODEL`, `ALFA_SMART_TURN_ORT`).
//! `ALFA_SMART_TURN_WAV` — nagranie 16 kHz mono PCM16; `ALFA_SMART_TURN_REF` — wzorzec z ONNX
//! Runtime w Pythonie (`alfa-glos-proba/proba_turn.py`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use voice_turn_impl::{AudioScorer, SidecarCfg, SidecarScorer};

fn wav_samples(path: &str) -> Vec<f32> {
    let bytes = std::fs::read(path).unwrap();
    let data = bytes
        .windows(4)
        .position(|w| w == b"data")
        .map(|i| i + 8)
        .unwrap();
    bytes[data..]
        .chunks_exact(2)
        .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
        .collect()
}

#[test]
fn real_model_matches_the_reference() {
    let var = |name: &str| std::env::var(name).ok();
    let (Some(exe), Some(model), Some(ort), Some(wav)) = (
        var("ALFA_SMART_TURN_EXE"),
        var("ALFA_SMART_TURN_MODEL"),
        var("ALFA_SMART_TURN_ORT"),
        var("ALFA_SMART_TURN_WAV"),
    ) else {
        return;
    };
    let mut log = std::io::stderr();
    let scorer = SidecarScorer::new(SidecarCfg::new(
        PathBuf::from(exe),
        PathBuf::from(model),
        PathBuf::from(ort),
    ));
    let audio = wav_samples(&wav);
    let reference: Option<serde_json::Value> = var("ALFA_SMART_TURN_REF")
        .map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap());
    let cuts = [audio.len(), audio.len() * 2 / 3, audio.len() / 2];
    for (i, cut) in cuts.into_iter().enumerate() {
        let t = Instant::now();
        let p = scorer.score(&audio[..cut]).unwrap();
        writeln!(
            log,
            "{:.1} s nagrania -> {p:.4} w {} ms",
            cut as f32 / 16_000.0,
            t.elapsed().as_millis()
        )
        .unwrap();
        assert!((0.0..=1.0).contains(&p));
        if let Some(r) = &reference {
            let want = r["cuts"][i]["probability"].as_f64().unwrap() as f32;
            assert!((p - want).abs() < 0.02, "wzorzec {want}, wynik {p}");
        }
    }
}
