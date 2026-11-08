//! Enkoder o budowie eksportu HF XLM-R (`CumSum` pozycji, uwaga wielogłowicowa z kształtami
//! liczonymi w grafie, `Erf`, rozłożona `LayerNorm`) w `tract` vs onnxruntime
//! (`fixtures/xlmr-like.references.json`, generator `fixtures/gen_xlmr_references.py`).

use std::io::Read;

use lib_embed::testkit::{XLMR_DIMS, write_files, xlmr_like_model_bytes};
use lib_embed::{EMBED_MANIFEST_FORMAT, EmbedManifest, Engine, FileRef, Pooling};
use serde_json::Value;

fn tokenizer_json() -> String {
    let gz = include_bytes!("../fixtures/xlmr-toy.tokenizer.json.gz");
    let mut text = String::new();
    flate2::read::GzDecoder::new(&gz[..])
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn engine(dir: &std::path::Path, pooled: bool) -> Engine {
    let model = xlmr_like_model_bytes(1502);
    let tok = tokenizer_json();
    let m = EmbedManifest {
        format: EMBED_MANIFEST_FORMAT.into(),
        id: "xlmr-like".into(),
        license: "CC0-1.0".into(),
        model: FileRef {
            path: "onnx/model.onnx".into(),
            sha256: lib_embed::manifest::sha256_hex(&model),
        },
        tokenizer: FileRef {
            path: "tokenizer.json".into(),
            sha256: lib_embed::manifest::sha256_hex(tok.as_bytes()),
        },
        dims: XLMR_DIMS,
        max_tokens: 64,
        pooling: if pooled {
            Pooling::Pooled
        } else {
            Pooling::Mean
        },
        normalize: true,
        query_prefix: String::new(),
        passage_prefix: String::new(),
        output: pooled.then(|| "pooler_output".to_owned()),
        batch_size: 8,
        ram_mb: 16,
        idle_unload_s: 0,
    };
    write_files(dir, &m, &model, tok.as_bytes()).unwrap();
    Engine::load(&m, dir).unwrap()
}

fn floats(v: &Value) -> Vec<f32> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap() as f32)
        .collect()
}

fn close(a: &[f32], b: &[f32], what: &str) {
    let worst = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f32, f32::max);
    assert!(a.len() == b.len() && worst < 1e-4, "{what}: {worst}");
}

#[test]
fn xlmr_like_export_matches_onnxruntime() {
    let refs: Value =
        serde_json::from_str(include_str!("../fixtures/xlmr-like.references.json")).unwrap();
    let cases = refs["cases"].as_array().unwrap();
    let texts: Vec<String> = cases
        .iter()
        .map(|c| c["text"].as_str().unwrap().to_owned())
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let mean = engine(dir.path(), false);
    for (c, text) in cases.iter().zip(&texts) {
        let ids: Vec<u32> = c["ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(mean.tokenize(text), ids, "{text}");
        close(
            &mean.embed(std::slice::from_ref(text)).unwrap()[0],
            &floats(&c["mean"]),
            text,
        );
    }
    // Wsad z wypełnieniem `<pad>` = 1: pozycje (`CumSum` po masce) i maska uwagi bez wpływu.
    for (v, c) in mean.embed(&texts).unwrap().iter().zip(cases) {
        close(v, &floats(&c["mean"]), "wsad");
    }
    let dir = tempfile::tempdir().unwrap();
    let pooler = engine(dir.path(), true);
    for (v, c) in pooler.embed(&texts).unwrap().iter().zip(cases) {
        close(v, &floats(&c["pooler"]), "pooler_output");
    }
}

fn rss_mb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

/// Pomiar czasu i RAM na modelu o kształcie `multilingual-e5-small` (12 warstw, 384 wym., słownik
/// 250 002; losowe wagi — wynik jak dla prawdziwego modelu fp32). Uruchamiać w `--release`:
/// `cargo test --release -p lib-embed --test it e5_small_shaped -- --ignored --nocapture`.
#[test]
#[ignore = "pomiar (~0,5 GB modelu, build --release)"]
fn e5_small_shaped_latency_and_ram() {
    use lib_embed::testkit::{XlmrShape, xlmr_model_bytes};
    use std::time::Instant;
    let shape = XlmrShape::e5_small();
    let started = Instant::now();
    let model = xlmr_model_bytes(shape);
    eprintln!(
        "model {} MB zbudowany w {:?}",
        model.len() >> 20,
        started.elapsed()
    );
    let tok = tokenizer_json();
    let dir = tempfile::tempdir().unwrap();
    let m = EmbedManifest {
        format: EMBED_MANIFEST_FORMAT.into(),
        id: "e5-small-shaped".into(),
        license: "CC0-1.0".into(),
        model: FileRef {
            path: "onnx/model.onnx".into(),
            sha256: lib_embed::manifest::sha256_hex(&model),
        },
        tokenizer: FileRef {
            path: "tokenizer.json".into(),
            sha256: lib_embed::manifest::sha256_hex(tok.as_bytes()),
        },
        dims: shape.dims,
        max_tokens: 512,
        pooling: Pooling::Mean,
        normalize: true,
        query_prefix: "query: ".into(),
        passage_prefix: "passage: ".into(),
        output: None,
        batch_size: 8,
        ram_mb: 640,
        idle_unload_s: 0,
    };
    write_files(dir.path(), &m, &model, tok.as_bytes()).unwrap();
    drop(model);
    let before = rss_mb();
    let started = Instant::now();
    let engine = Engine::load(&m, dir.path()).unwrap();
    eprintln!(
        "załadowano w {:?}; RSS {before:?} → {:?} MB",
        started.elapsed(),
        rss_mb()
    );
    let short = vec!["query: Kiedy mam wizytę u dentysty?".to_owned()];
    engine.embed(&short).unwrap();
    let mut times: Vec<_> = (0..7)
        .map(|_| {
            let t = Instant::now();
            engine.embed(&short).unwrap();
            t.elapsed()
        })
        .collect();
    times.sort();
    eprintln!("zapytanie (krótkie): mediana {:?}", times[3]);
    let batch: Vec<String> = (0..32)
        .map(|i| format!("passage: Fakt numer {i}: spotkanie zespołu marketingu odbywa się we wtorki o 10:00 w sali {i}."))
        .collect();
    let t = Instant::now();
    engine.embed(&batch).unwrap();
    eprintln!(
        "32 dokumenty: {:?} ({:?}/dok.)",
        t.elapsed(),
        t.elapsed() / 32
    );
    let long = vec![format!(
        "passage: {}",
        "Zażółć gęślą jaźń i pojedź do Gdańska. ".repeat(80)
    )];
    let t = Instant::now();
    engine.embed(&long).unwrap();
    eprintln!(
        "długi tekst ({} tokenów): {:?}; RSS po pracy {:?} MB",
        engine.tokenize(&long[0]).len(),
        t.elapsed(),
        rss_mb()
    );
}
