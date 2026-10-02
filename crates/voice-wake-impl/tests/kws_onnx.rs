//! Modele KWS przez `tract-onnx` na małych grafach zbudowanych w teście (bez pobierania modeli):
//! manifest z SHA-256 (podmieniony plik = odmowa), klasyfikator log-mel wykrywa „frazy” (tony)
//! z właściwą adresatką przez cały nasłuch z kontraktu, potok openWakeWord działa na stałych
//! kształtach. Prawdziwy model: `real_model` (`#[ignore]`, `ALFA_KWS_MODEL`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;

use common::onnx::{floats, int_attr, int64s, ints_attr, model, node, value};
use personas_contract::{PersonaId, builtin_personas};
use voice_audio_contract::synth::{sine, white_noise};
use voice_dsp_contract::{Fbank, FbankCfg};
use voice_wake_contract::{KwsParams, WakeError, WakeWordCfg, WakeWordListener};
use voice_wake_fake::BUILTIN_TONES;
use voice_wake_impl::kws::{KwsManifest, load_scorer, sha256_hex};

const MELS: usize = 40;
const FRAMES: usize = 100;

/// Klasyfikator: max po czasie log-mel → różnica pasma tonu frazy i średniej reszty → logit.
fn classifier() -> Vec<u8> {
    let fb = Fbank::new(FbankCfg::kaldi(MELS)).unwrap();
    let k = BUILTIN_TONES.len();
    let mut w = vec![0.0f32; MELS * k];
    for (j, (_, hz)) in BUILTIN_TONES.iter().enumerate() {
        let feats = fb.compute(&sine(*hz, 16_000, 0.2, 0.3));
        let row = &feats[5];
        let bin = (0..MELS)
            .max_by(|a, b| row[*a].total_cmp(&row[*b]))
            .unwrap();
        for m in 0..MELS {
            w[m * k + j] = if m == bin {
                1.0
            } else {
                -1.0 / (MELS - 1) as f32
            };
        }
    }
    model(
        value("feats", &[1, FRAMES as i64, MELS as i64]),
        value("logits", &[1, k as i64]),
        vec![
            node(
                "ReduceMax",
                &["feats"],
                "pooled",
                vec![ints_attr("axes", &[1]), int_attr("keepdims", 0)],
            ),
            node("MatMul", &["pooled", "w"], "proj", vec![]),
            node("Add", &["proj", "b"], "logits", vec![]),
        ],
        vec![
            floats("w", &[MELS as i64, k as i64], w),
            floats("b", &[k as i64], vec![-5.0; k]),
        ],
    )
}

fn write_logmel(dir: &Path, bytes: &[u8], hash: Option<&str>) -> std::path::PathBuf {
    std::fs::write(dir.join("kws.onnx"), bytes).unwrap();
    let manifest = serde_json::json!({
        "format": "alfa-kws-v1",
        "name": "test-tony",
        "license": "test",
        "labels": BUILTIN_TONES.map(|(l, _)| l),
        "model": {
            "kind": "log_mel",
            "model": {"path": "kws.onnx", "sha256": hash.map_or_else(|| sha256_hex(bytes), str::to_owned)},
            "n_mels": MELS, "frames": FRAMES, "step_frames": 8,
            "layout": "btf", "activation": "sigmoid"
        }
    });
    let path = dir.join("test.kws.json");
    std::fs::write(&path, manifest.to_string()).unwrap();
    path
}

fn phrase(hz: f32) -> Vec<f32> {
    let mut a = white_noise(4, 16_000, 0.001);
    a.extend(sine(hz, 16_000, 0.6, 0.3));
    a.extend(white_noise(5, 8_000, 0.001));
    a
}

#[test]
fn logmel_model_detects_phrases_with_addressee() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_logmel(dir.path(), &classifier(), None);
    let cfg = WakeWordCfg::from_personas(&builtin_personas(), 0.8);
    let expected = [
        PersonaId::alfa(),
        PersonaId::beta(),
        PersonaId::gama(),
        PersonaId::delta(),
    ];
    for ((_, hz), persona) in BUILTIN_TONES.iter().zip(expected) {
        let scorer = load_scorer(&path).unwrap();
        let mut l = WakeWordListener::new(&cfg, KwsParams::default(), scorer).unwrap();
        let mut hits = Vec::new();
        for block in phrase(*hz).chunks(1_600) {
            hits.extend(l.push(block).unwrap());
        }
        assert_eq!(hits.len(), 1, "{hz} Hz: {hits:?}");
        assert_eq!(hits[0].hit.persona, persona);
        assert!(hits[0].hit.at_ms >= 1_000 && hits[0].hit.at_ms <= 1_800);
    }
    let scorer = load_scorer(&path).unwrap();
    let mut l = WakeWordListener::new(&cfg, KwsParams::default(), scorer).unwrap();
    assert!(
        l.push(&white_noise(8, 16_000 * 10, 0.02))
            .unwrap()
            .is_none()
    );
}

#[test]
fn manifest_hash_and_paths_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_logmel(dir.path(), &classifier(), Some(&"0".repeat(64)));
    let e = load_scorer(&path).err().unwrap();
    assert!(
        matches!(&e, WakeError::Model(m) if m.contains("SHA-256")),
        "{e}"
    );
    let bad = r#"{"format":"alfa-kws-v1","name":"x","license":"x","labels":["hej alfa"],
        "model":{"kind":"log_mel","model":{"path":"../x.onnx","sha256":"00"},
        "n_mels":40,"frames":100,"step_frames":8,"layout":"btf","activation":"sigmoid"}}"#;
    assert!(KwsManifest::parse(bad).is_err());
    assert!(KwsManifest::parse(&bad.replace("alfa-kws-v1", "v0")).is_err());
    assert!(load_scorer(&dir.path().join("brak.kws.json")).is_err());
}

/// Atrapy modeli openWakeWord o właściwych kształtach.
fn oww_models() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mel = model(
        value("input", &[1, 1_760]),
        value("mel", &[1, 1, 9, 32]),
        vec![
            node("Slice", &["input", "st", "en", "ax"], "cut", vec![]),
            node("Reshape", &["cut", "shape"], "r", vec![]),
            node("Div", &["r", "scale"], "mel", vec![]),
        ],
        vec![
            int64s("st", vec![0]),
            int64s("en", vec![288]),
            int64s("ax", vec![1]),
            int64s("shape", vec![1, 1, 9, 32]),
            floats("scale", &[1], vec![32_768.0]),
        ],
    );
    let emb = model(
        value("x", &[1, 76, 32, 1]),
        value("e", &[1, 96]),
        vec![
            node(
                "ReduceMean",
                &["x"],
                "m",
                vec![ints_attr("axes", &[1, 3]), int_attr("keepdims", 0)],
            ),
            node("MatMul", &["m", "w"], "e", vec![]),
        ],
        vec![floats("w", &[32, 96], vec![0.01; 32 * 96])],
    );
    let cls = model(
        value("f", &[1, 16, 96]),
        value("p", &[1, 1, 1]),
        vec![
            node(
                "ReduceMean",
                &["f"],
                "m",
                vec![ints_attr("axes", &[1, 2]), int_attr("keepdims", 1)],
            ),
            node("Sigmoid", &["m"], "p", vec![]),
        ],
        vec![],
    );
    (mel, emb, cls)
}

#[test]
fn openwakeword_pipeline_runs_on_fixed_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let (mel, emb, cls) = oww_models();
    for (name, bytes) in [
        ("mel.onnx", &mel),
        ("emb.onnx", &emb),
        ("hej_alfa.onnx", &cls),
    ] {
        std::fs::write(dir.path().join(name), bytes).unwrap();
    }
    let f = |p: &str, b: &[u8]| serde_json::json!({"path": p, "sha256": sha256_hex(b)});
    let manifest = serde_json::json!({
        "format": "alfa-kws-v1", "name": "oww-test", "license": "Apache-2.0 (atrapa)",
        "labels": ["hej alfa"],
        "model": {"kind": "openwakeword", "melspectrogram": f("mel.onnx", &mel),
                  "embedding": f("emb.onnx", &emb), "classifiers": [f("hej_alfa.onnx", &cls)]}
    });
    let path = dir.path().join("oww.kws.json");
    std::fs::write(&path, manifest.to_string()).unwrap();
    let mut s = load_scorer(&path).unwrap();
    assert_eq!(s.labels(), ["hej alfa".to_owned()]);
    let out = s.push(&sine(500.0, 16_000, 1.0, 0.5)).unwrap();
    assert_eq!(out.len(), 12, "co 80 ms");
    assert_eq!(out[0].at_ms, 80);
    assert!(
        out.iter()
            .all(|k| k.scores.len() == 1 && (0.0..=1.0).contains(&k.scores[0]))
    );
    s.reset();
    assert_eq!(s.push(&[0.0; 1_280]).unwrap()[0].at_ms, 80);
}

/// Prawdziwy model (np. własny trening „Hej Alfa…”): `ALFA_KWS_MODEL=…/model.kws.json`,
/// opcjonalnie `ALFA_KWS_WAV=…/hej-alfa.wav` (16 kHz mono) — wypisuje wyniki.
#[test]
#[ignore = "wymaga modelu KWS spoza repo (ALFA_KWS_MODEL)"]
fn real_model() {
    let Ok(path) = std::env::var("ALFA_KWS_MODEL") else {
        eprintln!("ALFA_KWS_MODEL nie ustawione — pomijam");
        return;
    };
    let mut scorer = load_scorer(Path::new(&path)).unwrap();
    eprintln!("etykiety: {:?}", scorer.labels());
    if let Ok(wav) = std::env::var("ALFA_KWS_WAV") {
        let mut r = voice_wake_impl::eval::audio::WavReader::open(Path::new(&wav)).unwrap();
        let t0 = std::time::Instant::now();
        loop {
            let b = r.next_block(16_000).unwrap();
            if b.is_empty() {
                break;
            }
            for s in scorer.push(&b).unwrap() {
                eprintln!("{:>6} ms {:?}", s.at_ms, s.scores);
            }
        }
        eprintln!("{} ms audio w {:?}", r.duration_ms, t0.elapsed());
    }
}
