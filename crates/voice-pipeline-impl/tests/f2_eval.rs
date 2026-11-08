//! Zestaw F2 (evals/F2/): na CI — format manifestu, schematy, 3 próbki syntetyczne przez runner
//! offline (oś słów zamiast modelu, gramatyka komend i automat dialogu z kontraktów), zamrożenie.
//! Na maszynie użytkownika — `real_models_on_corpus` (`#[ignore]`, prawdziwy whisper.cpp).
//! Aktualizacja schematów: `UPDATE_SCHEMAS=1 cargo test -p voice-pipeline-impl --test f2_eval`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use voice_cmd_contract::{CommandKind, Grammar, GrammarRecognizer};
use voice_dialog_contract::default_machine;
use voice_pipeline_impl::eval::{
    self, Engines, ItemKind, ManifestEntry, PrefixStt, Split, TimedStt, WhisperCli, audio,
    freeze_list, manifest_schema, parse_manifest, results_schema, run_item, score,
    validate_manifest, verdicts, verify_frozen,
};
use voice_stt_contract::{Stt, Word};

fn f2_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../evals/F2")
}

fn samples() -> Vec<ManifestEntry> {
    let text = std::fs::read_to_string(f2_dir().join("samples/manifest.ndjson")).unwrap();
    parse_manifest(&text).unwrap()
}

fn snapshot(name: &str, generated: &serde_json::Value) {
    let path = f2_dir().join(name);
    if std::env::var_os("UPDATE_SCHEMAS").is_some() {
        let mut text = serde_json::to_string_pretty(generated).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        &on_disk,
        generated,
        "{} nieaktualny — UPDATE_SCHEMAS=1",
        path.display()
    );
}

#[test]
fn schemas_match_repo_files() {
    snapshot("manifest.schema.json", &manifest_schema());
    snapshot("results.schema.json", &results_schema());
}

#[test]
fn sample_manifest_is_valid() {
    let entries = samples();
    assert!(validate_manifest(&entries).is_empty());
    let kinds: Vec<ItemKind> = entries.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        [
            ItemKind::FreeSpeech,
            ItemKind::Command,
            ItemKind::Backchannel
        ]
    );
    assert_eq!(eval::select(&entries, Some(Split::Test)).len(), 2);
}

/// Oś słów jak z ASR: słowa transkrypcji rozłożone równo w mowie próbki.
fn timeline(e: &ManifestEntry) -> Vec<Word> {
    let spec = e.synth.unwrap();
    let text = e.transcript.clone().unwrap_or_default();
    let words: Vec<&str> = text.split_whitespace().collect();
    let step = spec.speech_ms / words.len().max(1) as u64;
    words
        .iter()
        .enumerate()
        .map(|(i, w)| Word {
            text: (*w).to_owned(),
            start_ms: (spec.lead_ms + i as u64 * step) as u32,
            end_ms: (spec.lead_ms + (i as u64 + 1) * step) as u32,
            confidence: 0.9,
        })
        .collect()
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("alfa-f2-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[tokio::test]
async fn synthetic_samples_through_offline_runner() {
    let entries = samples();
    let root = temp_dir("synth");
    for e in &entries {
        let path = root.join(&e.audio);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, audio::wav16(&audio::synth_audio(&e.synth.unwrap()))).unwrap();
        audio::check_audio(e, &root).unwrap();
    }
    let commands = GrammarRecognizer::new(Grammar::default_pl_en());
    let dialog = default_machine();
    let mut results = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let stt = TimedStt::new(timeline(e), "pl");
        let pcm = audio::load_item_audio(e, &root).unwrap();
        let engines = Engines {
            stt: &stt,
            commands: &commands,
            dialog: &dialog,
        };
        results.push(run_item(&engines, e, &pcm, i as u64 + 1).await.unwrap());
    }
    assert_eq!(
        results[0].hypothesis.as_deref(),
        entries[0].transcript.as_deref()
    );
    assert_eq!(results[1].command, Some(CommandKind::Stop));
    let reaction = results[1].reaction_ms.unwrap();
    assert!(reaction < 300, "reakcja {reaction} ms");
    assert_eq!(results[2].interrupted, Some(false), "„mhm” nie przerywa");
    let report = score(&entries, &results);
    assert_eq!(
        (report.wer(), report.stop_recall.value()),
        (Some(0.0), Some(1.0))
    );
    assert_eq!(report.backchannel_precision(), Some(1.0));
    assert!(
        verdicts(&report).iter().all(|v| v.pass != Some(false)),
        "{report:?}"
    );

    let list = freeze_list(&entries, &root).unwrap();
    assert_eq!(list.lines().count(), 3, "manifest + 2 pliki test");
    assert!(verify_frozen(&entries, &root, &list).is_empty());
    std::fs::remove_dir_all(&root).unwrap();
}

fn env_path(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("ustaw {name} (evals/F2/README.md)"))
}

/// Przebieg na prawdziwym modelu i korpusie (bramka #3). Zmienne: `ALFA_F2_MANIFEST`,
/// `ALFA_F2_AUDIO_ROOT`, `ALFA_WHISPER_CLI`, `ALFA_WHISPER_MODEL`; opcjonalnie `ALFA_F2_SPLIT`
/// (`dev`/`test`, domyślnie `test`), `ALFA_F2_FROZEN` (lista zamrożenia sprawdzana przed oceną)
/// i `ALFA_F2_MODE=timeline` (szybki tryb: jedna oś słów na pozycję zamiast modelu na prefiksach).
#[tokio::test]
#[ignore = "wymaga korpusu nagrań i whisper.cpp na maszynie użytkownika"]
async fn real_models_on_corpus() {
    let manifest = std::fs::read_to_string(env_path("ALFA_F2_MANIFEST")).unwrap();
    let all = parse_manifest(&manifest).unwrap();
    assert_eq!(validate_manifest(&all), Vec::<String>::new());
    let root = env_path("ALFA_F2_AUDIO_ROOT");
    if let Some(frozen) = std::env::var_os("ALFA_F2_FROZEN") {
        let list = std::fs::read_to_string(frozen).unwrap();
        assert_eq!(verify_frozen(&all, &root, &list), Vec::<String>::new());
    }
    let split = match std::env::var("ALFA_F2_SPLIT").as_deref() {
        Ok("dev") => Split::Dev,
        _ => Split::Test,
    };
    let timeline = std::env::var("ALFA_F2_MODE").as_deref() == Ok("timeline");
    let entries = eval::select(&all, Some(split));
    let cli = WhisperCli {
        exe: env_path("ALFA_WHISPER_CLI"),
        model: env_path("ALFA_WHISPER_MODEL"),
        lang: "pl".into(),
        work: root.join(".work"),
    };
    let commands = GrammarRecognizer::new(Grammar::default_pl_en());
    let dialog = default_machine();
    let mut results = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let stt: Box<dyn Stt> = if timeline {
            Box::new(TimedStt::new(
                cli.file_words(&root.join(&e.audio), e.segment).unwrap(),
                "pl",
            ))
        } else {
            Box::new(PrefixStt::new(cli.clone()))
        };
        let pcm = audio::load_item_audio(e, Path::new(&root)).unwrap();
        let engines = Engines {
            stt: stt.as_ref(),
            commands: &commands,
            dialog: &dialog,
        };
        results.push(run_item(&engines, e, &pcm, i as u64 + 1).await.unwrap());
    }
    let report = score(&entries, &results);
    eprintln!("{}", eval::to_markdown(&report));
    let failed: Vec<_> = verdicts(&report)
        .into_iter()
        .filter(|v| v.pass == Some(false))
        .collect();
    assert!(failed.is_empty(), "{failed:#?}");
}

fn cli(args: &[&str]) -> (bool, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_alfa-voice-eval"))
        .args(args)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

#[test]
fn cli_check_synth_freeze_verify_score() {
    let manifest = f2_dir().join("samples/manifest.ndjson");
    let manifest = manifest.to_str().unwrap();
    let dir = temp_dir("cli");
    let audio = dir.join("audio");
    let audio_s = audio.to_str().unwrap();
    assert_eq!(
        cli(&["check", manifest]),
        (true, "manifest OK: 3 pozycji (dev 1, test 2)\n".into())
    );
    assert!(cli(&["synth", manifest, audio_s]).0);
    assert!(cli(&["check", manifest, "--audio-root", audio_s]).0);
    let (ok, list) = cli(&["freeze", manifest, "--audio-root", audio_s]);
    assert!(ok && list.contains("  manifest:test\n"), "{list}");
    let frozen = dir.join("test.sha256");
    std::fs::write(&frozen, &list).unwrap();
    assert!(
        cli(&[
            "verify",
            manifest,
            "--audio-root",
            audio_s,
            "--frozen",
            frozen.to_str().unwrap()
        ])
        .0
    );
    std::fs::write(audio.join("synthetic/syn-stop-02.wav"), b"RIFF").unwrap();
    let (ok, text) = cli(&[
        "verify",
        manifest,
        "--audio-root",
        audio_s,
        "--frozen",
        frozen.to_str().unwrap(),
    ]);
    assert!(
        !ok && text.contains("plik zmieniony po zamrożeniu"),
        "{text}"
    );
    let results = dir.join("results.ndjson");
    std::fs::write(
        &results,
        "{\"id\":\"syn-stop-02\",\"command\":\"stop\",\"reaction_ms\":350}\n",
    )
    .unwrap();
    let (ok, report) = cli(&[
        "score",
        manifest,
        results.to_str().unwrap(),
        "--split",
        "test",
    ]);
    assert!(
        !ok && report.contains("| F2-04 | reakcja < 300 ms (p95 350 ms) | 0.0 %"),
        "{report}"
    );
    assert!(!cli(&["score", manifest, results.to_str().unwrap(), "--split", "x"]).0);
    assert!(!cli(&["nieznane"]).0);
    std::fs::remove_dir_all(&dir).unwrap();
}
