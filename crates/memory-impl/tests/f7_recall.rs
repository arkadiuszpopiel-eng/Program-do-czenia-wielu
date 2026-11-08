//! Zestaw recall@5 (ACCEPTANCE F7-02, `evals/F7/recall/`): na CI — format, zgodność plików
//! zestawu syntetycznego z generatorem, schematy, wynik na `HashEmbedder` (raportowany,
//! nieblokujący) i ścieżka embeddera ONNX na zabawkowym modelu (`lib-embed` testkit: manifest,
//! SHA-256, tokenizer, `tract`, pooling — ta sama droga co prawdziwy model).
//!
//! Prawdziwy embedder: `ALFA_F7_EMBEDDER=<katalog>/embed.json` (manifest `alfa-embed-v1`, np.
//! `multilingual-e5-small`; `crates/lib-embed/README.md`) — raport na zestawie syntetycznym i na
//! korpusie użytkownika (`ALFA_F7_CORPUS=<katalog z corpus.ndjson i queries.ndjson>`).
//! `ALFA_F7_STRICT=1` wymusza próg 0,85 i ≥ 200 zapytań — tylko z `ALFA_F7_EMBEDDER`.
//! Model ONNX w buildzie debug jest wolny: `cargo test --release -p memory-impl --test f7_recall`.
//! Raporty: `<temp>/alfa-f7-recall-<etykieta>.report.json` + wyniki per zapytanie `.ndjson`.
//! Aktualizacja plików zestawu: `ALFA_F7_WRITE=1 cargo test -p memory-impl --test f7_recall`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use lib_embed::{Embedder, OnnxEmbedder};
use memory_contract::MemoryService;
use memory_impl::eval::{self, RecallSet};

fn recall_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../evals/F7/recall")
}

fn check_file(path: &PathBuf, expected: &str) {
    if std::env::var("ALFA_F7_WRITE").is_ok_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, expected).unwrap();
    }
    let actual = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("brak {} — uruchom z ALFA_F7_WRITE=1", path.display()));
    assert_eq!(
        actual.replace("\r\n", "\n"),
        expected,
        "{} różni się od generatora (zestaw zamrożony: zmiana wymaga przeglądu człowieka)",
        path.display()
    );
}

#[test]
fn synthetic_files_match_generator_and_schemas() {
    let set = eval::synthetic_set();
    assert!(set.queries.len() >= eval::MIN_QUERIES);
    let (corpus, queries) = set.to_ndjson();
    let dir = recall_dir();
    check_file(&dir.join("synthetic/corpus.ndjson"), &corpus);
    check_file(&dir.join("synthetic/queries.ndjson"), &queries);
    let pretty = |v: serde_json::Value| serde_json::to_string_pretty(&v).unwrap() + "\n";
    check_file(
        &dir.join("corpus.schema.json"),
        &pretty(eval::corpus_schema()),
    );
    check_file(
        &dir.join("queries.schema.json"),
        &pretty(eval::queries_schema()),
    );
    let parsed = RecallSet::parse(
        &std::fs::read_to_string(dir.join("synthetic/corpus.ndjson")).unwrap(),
        &std::fs::read_to_string(dir.join("synthetic/queries.ndjson")).unwrap(),
    )
    .unwrap();
    assert_eq!(parsed, set);
    let bad = RecallSet::parse(
        &corpus,
        "{\"id\":\"q1\",\"query\":\"x\",\"expected\":[\"nie-ma\"],\"scopes\":[\"global\"],\"kind\":\"pytanie\"}\n",
    );
    assert!(bad.unwrap_err().to_string().contains("queries.ndjson:1"));
}

/// Uruchamia zestaw, wypisuje raport i zapisuje go (`.report.json`) z wynikami (`.ndjson`).
fn report(
    label: &str,
    embedder: &str,
    memory: &dyn MemoryService,
    set: &RecallSet,
) -> eval::RecallReport {
    let started = Instant::now();
    let loaded = eval::load(memory, set).unwrap();
    let loaded_in = started.elapsed();
    let (report, results) = eval::run(memory, set, &loaded, eval::K).unwrap();
    eprintln!(
        "F7-02 recall@{} [{label}, {embedder}]: {:.3} (trafienie@1 {:.3}, MRR {:.3}, zapytań {}, próg {} — na prawdziwym embedderze; ładowanie {:?}, zapytania {:?})",
        report.k,
        report.recall_at_k,
        report.hit_at_1,
        report.mrr,
        report.queries,
        eval::RECALL_THRESHOLD,
        loaded_in,
        started.elapsed() - loaded_in,
    );
    for (kind, value) in &report.by_kind {
        eprintln!("  {kind}: {value:.3}");
    }
    eprintln!("  chybione: {}", report.misses.len());
    let base = std::env::temp_dir().join(format!("alfa-f7-recall-{label}"));
    let lines: String = results
        .iter()
        .map(|r| serde_json::to_string(r).unwrap() + "\n")
        .collect();
    std::fs::write(base.with_extension("ndjson"), lines).unwrap();
    let json = serde_json::json!({ "label": label, "embedder": embedder, "report": report });
    let path = base.with_extension("report.json");
    std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap() + "\n").unwrap();
    let back: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let parsed: eval::RecallReport = serde_json::from_value(back["report"].clone()).unwrap();
    assert_eq!(
        (parsed.k, parsed.queries, &parsed.misses),
        (report.k, report.queries, &report.misses),
        "format raportu (round-trip)"
    );
    assert!((parsed.recall_at_k - report.recall_at_k).abs() < 1e-9);
    report
}

/// Embedder z manifestu (ta sama ścieżka dla prawdziwego i zabawkowego modelu).
fn onnx_embedder(manifest: &Path) -> Arc<OnnxEmbedder> {
    let embedder = OnnxEmbedder::from_manifest_file(manifest)
        .unwrap_or_else(|e| panic!("manifest {}: {e}", manifest.display()))
        .idle_unload(None)
        .spawn()
        .unwrap();
    let started = Instant::now();
    embedder
        .preload()
        .unwrap_or_else(|e| panic!("model {}: {e}", manifest.display()));
    eprintln!(
        "embedder {} ({} wym.) załadowany w {:?}",
        embedder.model_id(),
        embedder.dims(),
        started.elapsed()
    );
    Arc::new(embedder)
}

fn env_embedder() -> Option<Arc<OnnxEmbedder>> {
    std::env::var("ALFA_F7_EMBEDDER")
        .ok()
        .map(|p| onnx_embedder(Path::new(&p)))
}

fn strict() -> bool {
    std::env::var("ALFA_F7_STRICT").is_ok_and(|v| v == "1")
}

fn enforce(set: &RecallSet, r: &eval::RecallReport) {
    assert!(
        set.is_acceptance_sized(),
        "zestaw akceptacyjny: ≥ {} zapytań",
        eval::MIN_QUERIES
    );
    assert!(
        r.passes(eval::RECALL_THRESHOLD),
        "recall@5 {:.3} < {}",
        r.recall_at_k,
        eval::RECALL_THRESHOLD
    );
}

#[test]
fn synthetic_recall_at_5_reported() {
    let set = eval::synthetic_set();
    let hybrid = common::stack();
    let r = report(
        "hash-embedder-hybryda",
        "fake-hash-ngram-64",
        hybrid.memory.as_ref(),
        &set,
    );
    assert!(r.recall_at_k > 0.0 && r.queries == set.queries.len());
    let lexical = memory_fake::service();
    let r = report("atrapa-leksykalna", "memory-fake", &lexical, &set);
    assert!(r.recall_at_k > 0.0);
    if let Some(embedder) = env_embedder() {
        let id = embedder.model_id().to_owned();
        let stack = common::stack_with_embedder(embedder);
        let r = report("onnx-syntetyczny", &id, stack.memory.as_ref(), &set);
        if strict() {
            enforce(&set, &r);
        }
    }
}

/// Ścieżka „z atrapą” na CI: zabawkowy enkoder ONNX z manifestem przez ten sam kod co
/// `ALFA_F7_EMBEDDER` (wynik raportowany — wagi zabawkowe, nie miara jakości).
#[test]
fn onnx_embedder_path_with_toy_model() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = lib_embed::testkit::write_toy_model(dir.path()).unwrap();
    let embedder = onnx_embedder(&manifest);
    assert_eq!(embedder.dims(), lib_embed::testkit::TOY_DIMS);
    let id = embedder.model_id().to_owned();
    let stack = common::stack_with_embedder(embedder.clone());
    let set = eval::synthetic_set();
    let r = report("onnx-zabawkowy", &id, stack.memory.as_ref(), &set);
    assert_eq!(r.queries, set.queries.len());
    assert!(r.recall_at_k > 0.0 && r.recall_at_k <= 1.0);
    let stats = embedder.stats();
    assert!(stats.loaded && stats.texts >= (set.corpus.len() + set.queries.len()) as u64);
}

#[test]
fn user_corpus_when_provided() {
    let Ok(dir) = std::env::var("ALFA_F7_CORPUS") else {
        eprintln!(
            "ALFA_F7_CORPUS nieustawione — pomijam korpus użytkownika (evals/F7/recall/README.md)"
        );
        return;
    };
    let dir = PathBuf::from(dir);
    let set = RecallSet::parse(
        &std::fs::read_to_string(dir.join("corpus.ndjson")).unwrap(),
        &std::fs::read_to_string(dir.join("queries.ndjson")).unwrap(),
    )
    .unwrap_or_else(|e| panic!("format korpusu: {e}"));
    let embedder = env_embedder();
    assert!(
        embedder.is_some() || !strict(),
        "ALFA_F7_STRICT=1 wymaga prawdziwego embeddera (ALFA_F7_EMBEDDER)"
    );
    let (r, _stack) = match embedder {
        Some(e) => {
            let id = e.model_id().to_owned();
            let stack = common::stack_with_embedder(e);
            let r = report("korpus-uzytkownika", &id, stack.memory.as_ref(), &set);
            (r, stack)
        }
        None => {
            let stack = common::stack();
            let r = report(
                "korpus-uzytkownika",
                "fake-hash-ngram-64",
                stack.memory.as_ref(),
                &set,
            );
            (r, stack)
        }
    };
    if strict() {
        enforce(&set, &r);
    }
}
