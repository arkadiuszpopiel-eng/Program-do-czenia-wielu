//! Zestaw recall@5 (ACCEPTANCE F7-02, `evals/F7/recall/`): na CI — format, zgodność plików
//! zestawu syntetycznego z generatorem, schematy, wynik na `HashEmbedder` (raportowany,
//! nieblokujący). Korpus użytkownika: `ALFA_F7_CORPUS=<katalog z corpus.ndjson i queries.ndjson>`;
//! `ALFA_F7_STRICT=1` wymusza próg 0,85 (tylko z prawdziwym embedderem — kompozycja `app-*`).
//! Aktualizacja plików zestawu: `ALFA_F7_WRITE=1 cargo test -p memory-impl --test f7_recall`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;

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

fn report(label: &str, memory: &dyn MemoryService, set: &RecallSet) -> eval::RecallReport {
    let loaded = eval::load(memory, set).unwrap();
    let (report, results) = eval::run(memory, set, &loaded, eval::K).unwrap();
    eprintln!(
        "F7-02 recall@{} [{label}]: {:.3} (trafienie@1 {:.3}, MRR {:.3}, zapytań {}, próg {} — na prawdziwym embedderze)",
        report.k,
        report.recall_at_k,
        report.hit_at_1,
        report.mrr,
        report.queries,
        eval::RECALL_THRESHOLD
    );
    for (kind, value) in &report.by_kind {
        eprintln!("  {kind}: {value:.3}");
    }
    eprintln!("  chybione: {}", report.misses.len());
    let out = std::env::temp_dir().join(format!("alfa-f7-recall-{label}.ndjson"));
    let lines: String = results
        .iter()
        .map(|r| serde_json::to_string(r).unwrap() + "\n")
        .collect();
    std::fs::write(&out, lines).unwrap();
    report
}

#[test]
fn synthetic_recall_at_5_reported() {
    let set = eval::synthetic_set();
    let hybrid = common::stack();
    let r = report("hash-embedder-hybryda", hybrid.memory.as_ref(), &set);
    assert!(r.recall_at_k > 0.0 && r.queries == set.queries.len());
    let lexical = memory_fake::service();
    let r = report("atrapa-leksykalna", &lexical, &set);
    assert!(r.recall_at_k > 0.0);
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
    let stack = common::stack();
    let r = report("korpus-uzytkownika", stack.memory.as_ref(), &set);
    if std::env::var("ALFA_F7_STRICT").is_ok_and(|v| v == "1") {
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
}
