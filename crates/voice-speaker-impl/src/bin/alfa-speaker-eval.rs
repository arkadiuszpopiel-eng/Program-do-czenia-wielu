//! `alfa-speaker-eval` — weryfikacja mówcy F5-07/F5-08 (evals/F5/voice/README.md §2):
//! sprawdzenie manifestu, rejestracja właściciela w profilu tymczasowym (w pamięci), próby
//! właściciela i obcych (Common Voice PL, TTS) → EER, FAR/FRR przy progach, próg dla FAR ≤ 0,1%.
//! Lokalnie, bez sieci; nie dotyka zapisanego profilu użytkownika.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use voice_speaker_contract::SpeakerCfg;
use voice_speaker_impl::OnnxSpeakerModel;
use voice_speaker_impl::eval::{manifest_schema, parse_manifest, run, validate_manifest};

const USAGE: &str = "\
użycie:
  alfa-speaker-eval check <manifest>
  alfa-speaker-eval run   <manifest> --model PLIK.speaker.json [--audio-root KATALOG]
                          [--split test] [--standard 0.45] [--strict 0.62] [--out próby.ndjson]
  alfa-speaker-eval schema";

fn emit(line: &str) -> Result<(), String> {
    writeln!(std::io::stdout().lock(), "{line}").map_err(|e| e.to_string())
}

fn main_inner(raw: Vec<String>) -> Result<bool, String> {
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
    let cmd = pos.first().map(String::as_str);
    if cmd == Some("schema") {
        emit(&serde_json::to_string_pretty(&manifest_schema()).map_err(|e| e.to_string())?)?;
        return Ok(true);
    }
    let manifest = pos.get(1).ok_or(USAGE)?;
    let text = std::fs::read_to_string(manifest).map_err(|e| format!("{manifest}: {e}"))?;
    let items = parse_manifest(&text)?;
    let problems = validate_manifest(&items);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    match cmd {
        Some("check") => {
            emit(&format!("pozycji: {}", items.len()))?;
            Ok(true)
        }
        Some("run") => {
            let num = |k: &str, d: f32| -> Result<f32, String> {
                opts.get(k)
                    .map_or(Ok(d), |v| v.parse().map_err(|_| format!("--{k}: liczba")))
            };
            let cfg = SpeakerCfg {
                threshold_standard: num("standard", SpeakerCfg::default().threshold_standard)?,
                threshold_strict: num("strict", SpeakerCfg::default().threshold_strict)?,
                ..SpeakerCfg::default()
            };
            let model_path = opts.get("model").ok_or("--model PLIK.speaker.json")?;
            let model = OnnxSpeakerModel::load(Path::new(model_path)).map_err(|e| e.to_string())?;
            let root = PathBuf::from(opts.get("audio-root").map_or(".", String::as_str));
            let split = opts.get("split").map_or("test", String::as_str);
            let (report, trials) = run(&items, &root, model, cfg, split)?;
            if let Some(out) = opts.get("out") {
                let lines: Vec<String> = trials
                    .iter()
                    .map(|t| serde_json::to_string(t).unwrap_or_default())
                    .collect();
                std::fs::write(out, lines.join("\n") + "\n").map_err(|e| format!("{out}: {e}"))?;
            }
            emit(&serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?)?;
            Ok(report.f5_07_ok && report.f5_08_ok && report.sufficient_impostors)
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    match main_inner(std::env::args().skip(1).collect()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(e) => {
            let _ = writeln!(std::io::stderr().lock(), "{e}");
            ExitCode::FAILURE
        }
    }
}
