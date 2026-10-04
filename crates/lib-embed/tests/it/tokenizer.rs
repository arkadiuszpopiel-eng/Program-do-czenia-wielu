//! Zgodność tokenizera z HF `tokenizers` 0.23.2 na wektorach referencyjnych.
//!
//! Fixture: tokenizer w stylu XLM-R (`multilingual-e5-*`) — Unigram SentencePiece wytrenowany na polskich
//! tekstach repo (1500 kawałków, pełne reguły `nmt_nfkc` w `precompiled_charsmap`), tokeny `<s> <pad> </s>
//! <unk>` + `<mask>` (`lstrip`). Warianty: „new” (`Precompiled` + `Replace(" {2,}")`, `Metaspace`
//! `prepend_scheme = always`, `TemplateProcessing`) i „legacy” (jak starsze `xlm-roberta-base`:
//! `Precompiled`, `WhitespaceSplit` + `Metaspace` `add_prefix_space`, `RobertaProcessing`).
//! Referencje (`*.references.json`) wygenerowane w Pythonie (`tokenizers` 0.23.2, `sentencepiece` 0.2.2):
//! teksty PL z diakrytykami (NFC i NFD), pełna szerokość, ligatury, emoji, CJK, twarde spacje, tokeny
//! specjalne w tekście, obcięcie do 16 tokenów.

use std::io::Read;

use lib_embed::TextTokenizer;
use serde_json::Value;

fn fixture_json() -> Value {
    let gz = include_bytes!("../fixtures/xlmr-toy.tokenizer.json.gz");
    let mut text = String::new();
    flate2::read::GzDecoder::new(&gz[..])
        .read_to_string(&mut text)
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn legacy(mut v: Value) -> Value {
    let charsmap = v["normalizer"]["normalizers"][0]["precompiled_charsmap"].clone();
    v["normalizer"] = serde_json::json!({"type": "Precompiled", "precompiled_charsmap": charsmap});
    v["pre_tokenizer"] = serde_json::json!({"type": "Sequence", "pretokenizers": [
        {"type": "WhitespaceSplit"}, {"type": "Metaspace", "replacement": "▁", "add_prefix_space": true}]});
    v["post_processor"] = serde_json::json!({"type": "RobertaProcessing", "sep": ["</s>", 2], "cls": ["<s>", 0],
        "trim_offsets": true, "add_prefix_space": true});
    v
}

fn check(variant: &str, tokenizer: &TextTokenizer) {
    let refs: Value =
        serde_json::from_str(include_str!("../fixtures/xlmr-toy.references.json")).unwrap();
    let cases = refs[variant].as_array().unwrap();
    assert!(cases.len() >= 50);
    let mut failures = Vec::new();
    for case in cases {
        let text = case["text"].as_str().unwrap();
        let max = case["max"].as_u64().map_or(usize::MAX, |m| m as usize);
        let expected: Vec<u32> = case["ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i.as_u64().unwrap() as u32)
            .collect();
        let actual = tokenizer.encode(text, max);
        if actual != expected {
            failures.push(format!("{text:?} (max {max}): {actual:?} ≠ {expected:?}"));
        }
    }
    assert!(failures.is_empty(), "{variant}:\n{}", failures.join("\n"));
}

#[test]
fn matches_hf_tokenizers_new_style() {
    let tokenizer = TextTokenizer::from_json(&fixture_json().to_string()).unwrap();
    assert_eq!(tokenizer.vocab_size(), 1502);
    assert_eq!(tokenizer.pad_id(), 1);
    check("new", &tokenizer);
}

#[test]
fn matches_hf_tokenizers_legacy_style() {
    let tokenizer = TextTokenizer::from_json(&legacy(fixture_json()).to_string()).unwrap();
    check("legacy", &tokenizer);
}
