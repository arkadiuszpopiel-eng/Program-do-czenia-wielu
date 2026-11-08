//! `alfa-wake-eval` — słowa wywoławcze F5 (evals/F5/voice/README.md): sprawdzenie manifestu,
//! przebieg modelu KWS na pozytywach i nagraniach tła, FAR/dzień i FRR z przeglądem progów,
//! zamrożenie podziału test. Działa wyłącznie lokalnie — nic nie wysyła.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use personas_contract::builtin_personas;
use voice_wake_contract::eval::default_thresholds;
use voice_wake_contract::{KwsParams, WakeWordCfg, normalize_phrase};
use voice_wake_impl::eval::{
    Split, WakeItem, manifest_schema, outcomes, parse_manifest, score_items, summarize,
    validate_manifest,
};
use voice_wake_impl::kws::{load_scorer, sha256_hex};

const USAGE: &str = "\
użycie:
  alfa-wake-eval check  <manifest> [--audio-root KATALOG]
  alfa-wake-eval run    <manifest> --model PLIK.kws.json [--audio-root KATALOG]
                        [--split dev|test] [--threshold 0.8] [--out wyniki.ndjson]
  alfa-wake-eval freeze <manifest> --audio-root KATALOG
  alfa-wake-eval schema";

fn parse_args(raw: Vec<String>) -> Result<(Vec<String>, BTreeMap<String, String>), String> {
    let mut pos = Vec::new();
    let mut opts = BTreeMap::new();
    let mut it = raw.into_iter();
    while let Some(a) = it.next() {
        match a.strip_prefix("--") {
            Some(n) => {
                let v = it.next().ok_or_else(|| format!("--{n}: brak wartości"))?;
                opts.insert(n.to_owned(), v);
            }
            None => pos.push(a),
        }
    }
    Ok((pos, opts))
}

fn load(path: &str) -> Result<Vec<WakeItem>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let items = parse_manifest(&text)?;
    let problems = validate_manifest(&items);
    if problems.is_empty() {
        Ok(items)
    } else {
        Err(problems.join("\n"))
    }
}

/// Frazy: wbudowane persony + frazy pozytywów z manifestu (imiona z Kreatora).
fn phrases(items: &[WakeItem], threshold: f32) -> WakeWordCfg {
    let mut cfg = WakeWordCfg::from_personas(&builtin_personas(), threshold);
    for it in items {
        if let (Some(p), Some(id)) = (&it.phrase, it.persona_id()) {
            let norm = normalize_phrase(p);
            if !cfg.phrases.iter().any(|(q, _)| normalize_phrase(q) == norm) {
                cfg.phrases.push((p.clone(), id));
            }
        }
    }
    cfg
}

fn emit(line: &str) -> Result<(), String> {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{line}").map_err(|e| e.to_string())
}

fn run(pos: &[String], opts: &BTreeMap<String, String>) -> Result<bool, String> {
    let manifest = pos.get(1).ok_or(USAGE)?;
    let root = PathBuf::from(opts.get("audio-root").map_or(".", String::as_str));
    match pos.first().map(String::as_str) {
        Some("check") => {
            let items = load(manifest)?;
            let missing: Vec<_> = items
                .iter()
                .filter_map(|i| i.audio.as_ref())
                .filter(|a| !root.join(a).is_file())
                .collect();
            emit(&format!(
                "pozycji: {}, brak plików: {}",
                items.len(),
                missing.len()
            ))?;
            for m in &missing {
                emit(&format!("brak: {m}"))?;
            }
            Ok(missing.is_empty())
        }
        Some("run") => {
            let items = load(manifest)?;
            let split = match opts.get("split").map(String::as_str) {
                Some("dev") => Some(Split::Dev),
                Some("test") => Some(Split::Test),
                None => None,
                Some(o) => return Err(format!("--split {o}: dev|test")),
            };
            let threshold: f32 = opts.get("threshold").map_or(Ok(0.8), |t| {
                t.parse().map_err(|_| "--threshold: liczba".to_owned())
            })?;
            let model = opts.get("model").ok_or("--model PLIK.kws.json")?;
            let scorer = load_scorer(Path::new(model)).map_err(|e| e.to_string())?;
            let labels = scorer.labels().to_vec();
            let cfg = phrases(&items, threshold);
            let chosen: Vec<&WakeItem> = items
                .iter()
                .filter(|i| split.is_none_or(|s| i.split == s))
                .collect();
            let params = KwsParams::default();
            let scored = score_items(&chosen, &root, &cfg, params, scorer)?;
            let summary = summarize(
                &scored,
                &cfg,
                &labels,
                params,
                threshold,
                &default_thresholds(),
            )?;
            if let Some(out) = opts.get("out") {
                let lines: Vec<String> =
                    outcomes(&scored, &chosen, &cfg, &labels, params, threshold)?
                        .iter()
                        .map(|o| serde_json::to_string(o).unwrap_or_default())
                        .collect();
                std::fs::write(out, lines.join("\n") + "\n").map_err(|e| format!("{out}: {e}"))?;
            }
            emit(&serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?)?;
            Ok(summary.f5_05_far_ok && summary.f5_06_frr_ok && summary.sufficient)
        }
        Some("freeze") => {
            let items = load(manifest)?;
            for it in items.iter().filter(|i| i.split == Split::Test) {
                let line = serde_json::to_string(it).map_err(|e| e.to_string())?;
                emit(&format!(
                    "{}  manifest:{}",
                    sha256_hex(line.as_bytes()),
                    it.id
                ))?;
                if let Some(a) = &it.audio {
                    let bytes = std::fs::read(root.join(a)).map_err(|e| format!("{a}: {e}"))?;
                    emit(&format!("{}  {a}", sha256_hex(&bytes)))?;
                }
            }
            Ok(true)
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.first().map(String::as_str) == Some("schema") {
        let schema = serde_json::to_string_pretty(&manifest_schema()).unwrap_or_default();
        return match emit(&schema) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    let result = parse_args(raw).and_then(|(pos, opts)| run(&pos, &opts));
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(e) => {
            let _ = writeln!(std::io::stderr().lock(), "{e}");
            ExitCode::FAILURE
        }
    }
}
