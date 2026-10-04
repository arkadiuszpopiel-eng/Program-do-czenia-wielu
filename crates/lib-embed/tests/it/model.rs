//! Embedder na zabawkowym modelu (`testkit`): zgodność z onnxruntime + HF `tokenizers`
//! (`fixtures/toy-model.references.json`, generator `fixtures/gen_references.py`), niezależność od
//! wypełnienia we wsadzie, prefiksy E5, wyjście `pooled`, `token_type_ids`, SHA-256, dzierżawa
//! `model-residency`, zwalnianie po bezczynności i po odebraniu dzierżawy.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lib_embed::testkit::{
    TOY_DIMS, TOY_TOKENIZER_JSON, TOY_VOCAB, toy_manifest, toy_model_bytes, write_files,
    write_toy_model,
};
use lib_embed::{EmbedError, EmbedManifest, Embedder, Engine, OnnxEmbedder, Pooling};
use model_residency_contract::{ModelRole, Residency};
use search_contract::SearchError;
use serde_json::Value;

struct Case {
    text: String,
    ids: Vec<u32>,
    mean: Vec<f32>,
    pooled: Vec<f32>,
    mean_types: Vec<f32>,
}

fn cases() -> Vec<Case> {
    let v: Value =
        serde_json::from_str(include_str!("../fixtures/toy-model.references.json")).unwrap();
    assert_eq!(v["dims"].as_u64(), Some(TOY_DIMS as u64));
    let floats = |c: &Value, k: &str| -> Vec<f32> {
        c[k].as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap() as f32)
            .collect()
    };
    v["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| Case {
            text: c["text"].as_str().unwrap().to_owned(),
            ids: c["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as u32)
                .collect(),
            mean: floats(c, "mean"),
            pooled: floats(c, "pooled"),
            mean_types: floats(c, "mean_types"),
        })
        .collect()
}

fn assert_close(actual: &[f32], expected: &[f32], what: &str) {
    assert_eq!(actual.len(), expected.len(), "{what}: wymiar");
    let worst = actual
        .iter()
        .zip(expected)
        .map(|(a, e)| (a - e).abs())
        .fold(0.0_f32, f32::max);
    assert!(worst < 1e-4, "{what}: maks. różnica {worst}");
}

fn toy_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let manifest = write_toy_model(dir.path()).unwrap();
    (dir, manifest)
}

fn engine_with(dir: &Path, f: impl FnOnce(&mut EmbedManifest), types: bool) -> Engine {
    let model = toy_model_bytes(TOY_VOCAB, TOY_DIMS, types);
    let mut m = toy_manifest(&model, TOY_TOKENIZER_JSON.as_bytes());
    f(&mut m);
    write_files(dir, &m, &model, TOY_TOKENIZER_JSON.as_bytes()).unwrap();
    Engine::load(&m, dir).unwrap()
}

#[test]
fn toy_model_matches_onnxruntime_reference() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_with(dir.path(), |_| {}, false);
    let cases = cases();
    for c in &cases {
        assert_eq!(engine.tokenize(&c.text), c.ids, "tokenizer: {:?}", c.text);
        let single = engine.embed(std::slice::from_ref(&c.text)).unwrap();
        assert_close(&single[0], &c.mean, &c.text);
    }
    // Jeden wsad z tekstami różnej długości: wypełnienie i maska nie zmieniają wyniku.
    let texts: Vec<String> = cases.iter().map(|c| c.text.clone()).collect();
    let batched = engine.embed(&texts).unwrap();
    for (v, c) in batched.iter().zip(&cases) {
        assert_close(v, &c.mean, "wsad");
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }
    assert!(engine.embed(&[]).unwrap().is_empty());
}

#[test]
fn pooled_output_and_token_type_ids() {
    let cases = cases();
    let texts: Vec<String> = cases.iter().map(|c| c.text.clone()).collect();
    let dir = tempfile::tempdir().unwrap();
    let pooled = engine_with(
        dir.path(),
        |m| {
            m.output = Some("pooled".into());
            m.pooling = Pooling::Pooled;
        },
        false,
    );
    for (v, c) in pooled.embed(&texts).unwrap().iter().zip(&cases) {
        assert_close(v, &c.pooled, "pooled");
    }
    let dir = tempfile::tempdir().unwrap();
    let types = engine_with(dir.path(), |_| {}, true);
    for (v, c) in types.embed(&texts).unwrap().iter().zip(&cases) {
        assert_close(v, &c.mean_types, "token_type_ids");
    }
    let dir = tempfile::tempdir().unwrap();
    let cls = engine_with(dir.path(), |m| m.pooling = Pooling::Cls, false);
    assert_eq!(cls.embed(&texts[..1]).unwrap()[0].len(), TOY_DIMS);
}

#[test]
fn wrong_dims_or_output_fail_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_with(dir.path(), |m| m.dims = 16, false);
    let err = engine.embed(&["tekst".to_owned()]).unwrap_err();
    assert!(
        matches!(err, EmbedError::Model(ref r) if r.contains("kształt")),
        "{err}"
    );
    let model = toy_model_bytes(TOY_VOCAB, TOY_DIMS, false);
    let mut m = toy_manifest(&model, TOY_TOKENIZER_JSON.as_bytes());
    m.output = Some("nie-ma".into());
    write_files(dir.path(), &m, &model, TOY_TOKENIZER_JSON.as_bytes()).unwrap();
    assert!(matches!(
        Engine::load(&m, dir.path()),
        Err(EmbedError::Model(_))
    ));
}

#[test]
fn embedder_applies_prefixes_and_splits_jobs() {
    let (_dir, manifest) = toy_dir();
    let embedder = OnnxEmbedder::from_manifest_file(&manifest)
        .unwrap()
        .spawn()
        .unwrap();
    let cases = cases();
    assert_eq!(embedder.dims(), TOY_DIMS);
    assert!(embedder.model_id().starts_with("toy-encoder@"));
    let passage = embedder
        .embed(&["Ulubiony kolor Karoliny to zielony."])
        .unwrap();
    assert_close(&passage[0], &cases[0].mean, "passage: …");
    let query = embedder
        .embed_query(&["Jaki kolor lubi Karolina?"])
        .unwrap();
    assert_close(&query[0], &cases[1].mean, "query: …");
    let many: Vec<String> = (0..70).map(|i| format!("tekst numer {i}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    let out = embedder.embed(&refs).unwrap();
    assert_eq!(out.len(), 70);
    assert_eq!(out[69], embedder.embed(&[refs[69]]).unwrap()[0]);
    let huge = "ż".repeat(100_000);
    assert_eq!(embedder.embed(&[&huge]).unwrap()[0].len(), TOY_DIMS);
    let stats = embedder.stats();
    assert!(stats.loaded && stats.loads == 1 && stats.texts >= 73);
}

#[test]
fn tampered_model_is_never_loaded() {
    let (dir, manifest) = toy_dir();
    let model_path = dir.path().join("model.onnx");
    let good = std::fs::read(&model_path).unwrap();
    let mut bad = good.clone();
    bad[100] ^= 0xFF;
    std::fs::write(&model_path, &bad).unwrap();
    let embedder = OnnxEmbedder::from_manifest_file(&manifest)
        .unwrap()
        .retry_after(Duration::ZERO)
        .spawn()
        .unwrap();
    assert!(matches!(embedder.preload(), Err(EmbedError::Hash { .. })));
    assert!(matches!(
        embedder.embed(&["x"]),
        Err(SearchError::Embedder { .. })
    ));
    std::fs::write(&model_path, &good).unwrap();
    embedder.preload().unwrap();
    assert_eq!(embedder.stats().loads, 1);
}

#[test]
fn failed_load_waits_before_retry() {
    let (dir, manifest) = toy_dir();
    std::fs::remove_file(dir.path().join("tokenizer.json")).unwrap();
    let embedder = OnnxEmbedder::from_manifest_file(&manifest)
        .unwrap()
        .retry_after(Duration::from_secs(3600))
        .spawn()
        .unwrap();
    assert!(matches!(embedder.preload(), Err(EmbedError::Io { .. })));
    std::fs::write(dir.path().join("tokenizer.json"), TOY_TOKENIZER_JSON).unwrap();
    // W oknie `retry_after` zwracany jest zapamiętany błąd (bez ponownego ładowania 470 MB).
    assert!(matches!(embedder.preload(), Err(EmbedError::Io { .. })));
    assert_eq!(embedder.stats().loads, 0);
}

fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn residency_lease_follows_model_lifetime() {
    let (_dir, manifest) = toy_dir();
    let residency = Arc::new(crate::residency::TableResidency::new(8_192));
    let embedder = OnnxEmbedder::from_manifest_file(&manifest)
        .unwrap()
        .residency(residency.clone())
        .idle_unload(Some(Duration::from_millis(30)))
        .spawn()
        .unwrap();
    assert!(residency.snapshot().leases.is_empty(), "leniwe ładowanie");
    embedder.embed(&["Gdzie jest paszport?"]).unwrap();
    let leases = residency.snapshot().leases;
    assert_eq!(leases.len(), 1);
    assert_eq!(leases[0].request.role, ModelRole::Embedder);
    assert_eq!(leases[0].request.cpu_ram_mb, 8);
    assert_eq!(leases[0].request.vram_mb, 0);
    assert!(
        wait_until(|| !embedder.stats().loaded),
        "zwolnienie po bezczynności"
    );
    assert!(wait_until(|| residency.snapshot().leases.is_empty()));
    embedder.embed(&["ponownie"]).unwrap();
    assert_eq!(embedder.stats().loads, 2);
}

#[test]
fn revoked_lease_unloads_model() {
    let (_dir, manifest) = toy_dir();
    let residency = Arc::new(crate::residency::TableResidency::new(8_192));
    let embedder = OnnxEmbedder::from_manifest_file(&manifest)
        .unwrap()
        .residency(residency.clone())
        .idle_unload(None)
        .spawn()
        .unwrap();
    embedder.preload().unwrap();
    residency.clock.advance_ms(301_000);
    assert_eq!(residency.reap_idle().len(), 1);
    assert!(wait_until(|| !embedder.stats().loaded));
    embedder.unload();
    embedder.embed(&["po odebraniu"]).unwrap();
    assert!(embedder.stats().loaded);
    embedder.unload();
    assert!(wait_until(|| !embedder.stats().loaded));
    assert!(wait_until(|| residency.snapshot().leases.is_empty()));
}

#[test]
fn residency_refusal_is_an_error_not_a_panic() {
    let (_dir, manifest) = toy_dir();
    let residency = Arc::new(crate::residency::TableResidency::new(4));
    let embedder = OnnxEmbedder::from_manifest_file(&manifest)
        .unwrap()
        .residency(residency)
        .spawn()
        .unwrap();
    assert!(matches!(embedder.preload(), Err(EmbedError::Residency(_))));
}

/// Prawdziwy model (poza repo): `ALFA_EMBED_MODEL=<katalog>/embed.json` (README: skąd wziąć
/// `multilingual-e5-small`). Sprawdza wymiar, normę, sens (pytanie bliżej pasującego faktu) i czas.
#[test]
#[ignore = "wymaga prawdziwego modelu (ALFA_EMBED_MODEL)"]
fn real_model_semantic_sanity() {
    let Ok(path) = std::env::var("ALFA_EMBED_MODEL") else {
        eprintln!("ALFA_EMBED_MODEL nieustawione — pomijam");
        return;
    };
    let started = Instant::now();
    let embedder = OnnxEmbedder::from_manifest_file(Path::new(&path))
        .unwrap()
        .spawn()
        .unwrap();
    embedder.preload().unwrap();
    eprintln!(
        "załadowano {} w {:?}",
        embedder.model_id(),
        started.elapsed()
    );
    let facts = [
        "Ulubiony kolor Karoliny to zielony.",
        "Paszport leży w górnej szufladzie biurka.",
        "Wizyta u dentysty jest 12 listopada o 9:00.",
    ];
    let t = Instant::now();
    let docs = embedder.embed(&facts).unwrap();
    eprintln!("3 dokumenty w {:?}", t.elapsed());
    let queries = embedder
        .embed_query(&[
            "Jaką barwę lubi Karolina?",
            "Gdzie trzymam dokumenty podróżne?",
            "Kiedy idę do stomatologa?",
        ])
        .unwrap();
    for (qi, q) in queries.iter().enumerate() {
        assert_eq!(q.len(), embedder.dims());
        let sims: Vec<f32> = docs
            .iter()
            .map(|d| d.iter().zip(q).map(|(a, b)| a * b).sum())
            .collect();
        eprintln!("zapytanie {qi}: {sims:?}");
        let best = sims
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i);
        assert_eq!(best, Some(qi));
    }
}
