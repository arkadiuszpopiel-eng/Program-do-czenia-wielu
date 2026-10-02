//! Runner FAR/FRR na próbkach syntetycznych CI (`evals/F5/voice/samples/wake.ndjson`) z modelem
//! atrapy (`ToneScorer`): format manifestu, walidacja, strumieniowy WAV, CLI, snapshot schematu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use personas_contract::builtin_personas;
use voice_audio_contract::AudioFormat;
use voice_audio_contract::synth::white_noise;
use voice_audio_contract::wav::{WavEncoding, encode_wav};
use voice_wake_contract::eval::default_thresholds;
use voice_wake_contract::{KeywordScorer, KwsParams, WakeWordCfg};
use voice_wake_fake::ToneScorer;
use voice_wake_impl::eval::audio::WavReader;
use voice_wake_impl::eval::{
    WakeItem, WakeItemKind, manifest_schema, outcomes, parse_manifest, score_items, summarize,
    validate_manifest,
};

fn evals() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/F5/voice")
}

fn samples() -> Vec<WakeItem> {
    parse_manifest(&std::fs::read_to_string(evals().join("samples/wake.ndjson")).unwrap()).unwrap()
}

#[test]
fn synthetic_samples_far_frr() {
    let items = samples();
    assert!(
        validate_manifest(&items).is_empty(),
        "{:?}",
        validate_manifest(&items)
    );
    let refs: Vec<&WakeItem> = items.iter().collect();
    let cfg = WakeWordCfg::from_personas(&builtin_personas(), 0.8);
    let scorer = ToneScorer::builtin();
    let labels = scorer.labels().to_vec();
    let params = KwsParams::default();
    let scored = score_items(&refs, Path::new("."), &cfg, params, Box::new(scorer)).unwrap();
    let s = summarize(&scored, &cfg, &labels, params, 0.8, &default_thresholds()).unwrap();
    eprintln!(
        "F5-05/06 (syntetyczne): FRR {:.3}, FAR/dzień {:.2} na {:.2} h tła, pozytywów {}",
        s.at_threshold.frr(),
        s.at_threshold.far_per_day(),
        s.at_threshold.background_hours,
        s.at_threshold.positives
    );
    assert_eq!((s.at_threshold.positives, s.at_threshold.detected), (8, 8));
    assert_eq!(s.at_threshold.false_alarms, 0);
    assert!(s.f5_05_far_ok && s.f5_06_frr_ok);
    assert!(
        !s.sufficient,
        "6 min tła i 8 pozytywów to nie zestaw akceptacyjny"
    );
    assert!(s.recommended.is_some() && s.sweep.len() == 14);
    let out = outcomes(&scored, &refs, &cfg, &labels, params, 0.8).unwrap();
    for o in &out {
        match o.kind {
            WakeItemKind::WakePositive => assert_eq!(o.hits.len(), 1, "{o:?}"),
            WakeItemKind::WakeBackground => {
                assert!(o.hits.is_empty() && o.max_score < 0.5, "{o:?}")
            }
        }
    }
}

#[test]
fn manifest_validation_catches_problems() {
    let line = |s: &str| format!("{s}\n");
    let text = [
        r#"{"id":"A","kind":"wake_positive","split":"dev","conditions":{"machine":"x","environment":"quiet","mic":"usb"},"audio":"a.wav"}"#,
        r#"{"id":"b","kind":"wake_background","split":"dev","conditions":{"machine":"x","environment":"tv","mic":"usb"},"audio":"../b.wav"}"#,
        r#"{"id":"b","kind":"wake_background","split":"test","conditions":{"machine":"x","environment":"tv","mic":"usb"}}"#,
        r#"{"id":"c","kind":"wake_positive","split":"test","persona":"alfa","segment":{"start_ms":5,"end_ms":5},"conditions":{"machine":"x","environment":"tv","mic":"usb"},"audio":"a.wav"}"#,
    ]
    .map(line)
    .concat();
    let items = parse_manifest(&text).unwrap();
    let p = validate_manifest(&items).join("\n");
    for needle in [
        "A: identyfikator",
        "A: pozytyw wymaga",
        "b: audio",
        "b: identyfikator powtórzony",
        "b: dokładnie jedno",
        "c: segment",
        "c: ten sam plik",
    ] {
        assert!(p.contains(needle), "{needle} ∉ {p}");
    }
    assert!(parse_manifest("{zły json").unwrap_err().contains("linia 1"));
}

#[test]
fn wav_is_read_in_blocks_and_format_checked() {
    let dir = tempfile::tempdir().unwrap();
    let pcm = white_noise(1, 40_000, 0.2);
    let ok = dir.path().join("ok.wav");
    std::fs::write(
        &ok,
        encode_wav(&pcm, AudioFormat::mono(16_000), WavEncoding::Pcm16),
    )
    .unwrap();
    let mut r = WavReader::open(&ok).unwrap();
    assert_eq!(r.duration_ms, 2_500);
    let mut n = 0;
    loop {
        let b = r.next_block(16_000).unwrap();
        if b.is_empty() {
            break;
        }
        assert!((b[0] - pcm[n]).abs() < 1e-3);
        n += b.len();
    }
    assert_eq!(n, 40_000);
    let f32wav = dir.path().join("f.wav");
    std::fs::write(
        &f32wav,
        encode_wav(&pcm, AudioFormat::mono(16_000), WavEncoding::Float32),
    )
    .unwrap();
    assert_eq!(
        WavReader::open(&f32wav).unwrap().next_block(4).unwrap()[1],
        pcm[1]
    );
    let stereo = dir.path().join("s.wav");
    std::fs::write(
        &stereo,
        encode_wav(&pcm, AudioFormat::stereo(48_000), WavEncoding::Pcm16),
    )
    .unwrap();
    assert!(
        WavReader::open(&stereo)
            .err()
            .unwrap()
            .contains("16 kHz mono")
    );
    std::fs::write(dir.path().join("x.wav"), b"nie wav").unwrap();
    assert!(WavReader::open(&dir.path().join("x.wav")).is_err());
}

#[test]
fn cli_check_and_schema_snapshot() {
    let exe = env!("CARGO_BIN_EXE_alfa-wake-eval");
    let manifest = evals().join("samples/wake.ndjson");
    let out = std::process::Command::new(exe)
        .args(["check", manifest.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("pozycji: 11, brak plików: 0"));
    let bad = std::process::Command::new(exe)
        .args(["nic"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    let schema = serde_json::to_string_pretty(&manifest_schema()).unwrap() + "\n";
    let path = evals().join("wake-manifest.schema.json");
    if std::env::var_os("ALFA_UPDATE_SCHEMAS").is_some() {
        std::fs::write(&path, &schema).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&path).unwrap_or_default(),
        schema,
        "schemat nieaktualny — uruchom z ALFA_UPDATE_SCHEMAS=1"
    );
}
