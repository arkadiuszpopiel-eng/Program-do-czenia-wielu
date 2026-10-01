use std::collections::BTreeMap;

use core_bus_contract::{Event, EventKind, Level};
use voice_cmd_contract::CommandKind;
use voice_dialog_contract::{EVENT_INTERRUPTED, InterruptIntent};

use super::manifest::{Conditions, Environment, Machine, Mic, Segment, SynthSpec};
use super::metrics::{normalize_words, percentile};
use super::*;

fn entry(id: &str, kind: ItemKind, split: Split) -> ManifestEntry {
    ManifestEntry {
        id: id.into(),
        audio: format!("16k/desktop/2026-10-01/{id}.wav"),
        kind,
        split,
        conditions: Conditions {
            machine: Machine::Desktop,
            environment: Environment::Quiet,
            mic: Mic::Usb,
        },
        transcript: Some("jutro o dziewiątej mam spotkanie".into()),
        segment: None,
        command: None,
        onset_ms: None,
        intent: None,
        heard_words: None,
        tts: None,
        synth: None,
        note: None,
    }
}

fn result(id: &str) -> ItemResult {
    ItemResult {
        id: id.into(),
        ..ItemResult::default()
    }
}

#[test]
fn wer_counts_substitutions_deletions_insertions() {
    assert_eq!(word_errors("Ala ma kota.", "ala ma kota"), (0, 3));
    assert_eq!(word_errors("ala ma kota", "ala ma psa"), (1, 3));
    assert_eq!(word_errors("ala ma kota", "ala kota"), (1, 3));
    assert_eq!(word_errors("ala ma kota", "ala ma bardzo kota"), (1, 3));
    assert_eq!(word_errors("", "coś"), (1, 0));
    assert_eq!(
        normalize_words("Źdźbło, ŻÓŁW!  i Teams'a"),
        ["źdźbło", "żółw", "i", "teams'a"]
    );
}

#[test]
fn percentile_nearest_rank() {
    assert_eq!(percentile(&[], 50.0), None);
    assert_eq!(percentile(&[30, 10, 20], 50.0), Some(20));
    assert_eq!(percentile(&[30, 10, 20], 95.0), Some(30));
    assert_eq!(percentile(&[5], 0.0), Some(5));
}

#[test]
fn manifest_parse_skips_comments_and_reports_lines() {
    let ok = serde_json::to_string(&entry("a1", ItemKind::FreeSpeech, Split::Dev)).unwrap();
    let text = format!("# komentarz\n\n{ok}\n");
    assert_eq!(parse_manifest(&text).unwrap().len(), 1);
    let bad = format!("{ok}\n{{\"id\":\"x\",\"nieznane\":1}}\n");
    let errors = parse_manifest(&bad).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].starts_with("linia 2:"), "{errors:?}");
}

#[test]
fn manifest_validation_finds_every_violation() {
    let good = entry("ok-1", ItemKind::FreeSpeech, Split::Dev);
    assert!(validate_manifest(std::slice::from_ref(&good)).is_empty());
    let mut bad_id = entry("Złe ID", ItemKind::FreeSpeech, Split::Dev);
    bad_id.audio = "/abs/x.wav".into();
    let mut leak = entry("ok-2", ItemKind::FreeSpeech, Split::Test);
    leak.audio = good.audio.clone();
    let mut cmd = entry("cmd-1", ItemKind::Command, Split::Dev);
    cmd.transcript = None;
    let intr = entry("int-1", ItemKind::Interruption, Split::Dev);
    let mut seg = entry("seg-1", ItemKind::Wake, Split::Dev);
    seg.segment = Some(Segment {
        start_ms: 500,
        end_ms: 500,
    });
    let mut synth = entry("syn-1", ItemKind::FreeSpeech, Split::Dev);
    synth.conditions.machine = Machine::Synthetic;
    let dup = good.clone();
    let errors = validate_manifest(&[good, bad_id, leak, cmd, intr, seg, synth, dup]);
    let expect = [
        "Złe ID: niepoprawny identyfikator",
        "Złe ID: audio musi być względną ścieżką do .wav",
        "ok-2: plik 16k/desktop/2026-10-01/ok-1.wav jednocześnie w dev i test",
        "cmd-1: brak transkrypcji referencyjnej",
        "cmd-1: komenda bez etykiety `command`",
        "int-1: przerwanie bez etykiety `intent`",
        "seg-1: segment pusty lub odwrócony",
        "syn-1: `synth` tylko i zawsze dla maszyny `synthetic`",
        "ok-1: powtórzony identyfikator",
    ];
    for e in expect {
        assert!(errors.iter().any(|x| x == e), "brak „{e}” w {errors:?}");
    }
    assert_eq!(errors.len(), expect.len(), "{errors:?}");
}

#[test]
fn select_filters_split() {
    let all = [
        entry("a", ItemKind::FreeSpeech, Split::Dev),
        entry("b", ItemKind::FreeSpeech, Split::Test),
    ];
    assert_eq!(select(&all, None).len(), 2);
    assert_eq!(select(&all, Some(Split::Test))[0].id, "b");
}

fn scored() -> (Vec<ManifestEntry>, Vec<ItemResult>) {
    let mut entries = vec![entry("w1", ItemKind::FreeSpeech, Split::Test)];
    let mut results = vec![ItemResult {
        hypothesis: Some("jutro o dziewiątej mam spotkania".into()),
        ..result("w1")
    }];
    for (i, (hit, ms)) in [(true, 180), (true, 250), (false, 0), (true, 320)]
        .into_iter()
        .enumerate()
    {
        let id = format!("c{i}");
        let mut e = entry(&id, ItemKind::Command, Split::Test);
        e.command = Some(if i % 2 == 0 {
            CommandKind::Stop
        } else {
            CommandKind::Cancel
        });
        results.push(ItemResult {
            command: hit.then_some(e.command).flatten(),
            reaction_ms: hit.then_some(ms),
            ..result(&id)
        });
        entries.push(e);
    }
    for (i, interrupted) in [false, false, false, true].into_iter().enumerate() {
        let id = format!("b{i}");
        entries.push(entry(&id, ItemKind::Backchannel, Split::Test));
        results.push(ItemResult {
            interrupted: Some(interrupted),
            ..result(&id)
        });
    }
    let labels = [
        (
            InterruptIntent::Correction,
            Some(InterruptIntent::Correction),
            true,
            7,
            8,
        ),
        (
            InterruptIntent::Correction,
            Some(InterruptIntent::Addition),
            true,
            7,
            9,
        ),
        (
            InterruptIntent::StopCancel,
            Some(InterruptIntent::StopCancel),
            false,
            3,
            3,
        ),
    ];
    for (i, (truth, got, interrupted, heard, seen)) in labels.into_iter().enumerate() {
        let id = format!("i{i}");
        let mut e = entry(&id, ItemKind::Interruption, Split::Test);
        e.intent = Some(truth);
        e.heard_words = Some(heard);
        entries.push(e);
        results.push(ItemResult {
            intent: got,
            interrupted: Some(interrupted),
            heard_words: Some(seen),
            ..result(&id)
        });
    }
    entries.push(entry("missing", ItemKind::Names, Split::Test));
    (entries, results)
}

#[test]
fn score_computes_every_metric() {
    let (entries, results) = scored();
    let r = score(&entries, &results);
    assert_eq!((r.wer_errors, r.wer_words), (1, 5));
    assert_eq!(r.stop_recall, Ratio { hits: 3, total: 4 });
    assert_eq!(r.stop_reactions_ms, [180, 250, 320]);
    assert_eq!(
        (
            r.backchannel_kept,
            r.backchannels_interrupting,
            r.interruptions_missed
        ),
        (3, 1, 1)
    );
    assert_eq!(r.backchannel_precision(), Some(0.75));
    assert_eq!(r.prefix, Ratio { hits: 2, total: 3 });
    let mut intents = BTreeMap::new();
    intents.insert("correction".to_owned(), Ratio { hits: 1, total: 2 });
    intents.insert("stop_cancel".to_owned(), Ratio { hits: 1, total: 1 });
    assert_eq!(r.intents, intents);
    assert_eq!(r.missing, ["missing"]);
    assert_eq!(score(&[], &[]).wer(), None);
}

#[test]
fn verdicts_apply_acceptance_thresholds() {
    let (entries, results) = scored();
    let mut r = score(&entries, &results);
    r.false_per_hour = Some(0.5);
    let v = verdicts(&r);
    let by = |metric: &str| {
        v.iter()
            .find(|x| x.metric.starts_with(metric))
            .map(|x| x.pass)
    };
    assert_eq!(by("WER PL"), Some(Some(false)));
    assert_eq!(by("recall"), Some(Some(false)));
    assert_eq!(by("reakcja"), Some(Some(false)));
    assert_eq!(by("precision"), Some(Some(false)));
    assert_eq!(by("fałszywe"), Some(Some(true)));
    assert_eq!(by("intencja `stop_cancel`"), Some(Some(true)));
    let md = to_markdown(&r);
    assert!(
        md.contains("| F2-06 | fałszywe przerwania | 0.50 / h | ≤ 1 / h | ✅ |"),
        "{md}"
    );
    assert!(md.contains("Bez wyniku: 1 pozycji (missing)"), "{md}");
    let empty = verdicts(&F2Report::default());
    assert!(
        empty
            .iter()
            .all(|x| x.pass.is_none() && x.value == "brak danych")
    );
}

#[test]
fn results_round_trip_and_reject_unknown_fields() {
    let (_, results) = scored();
    let text = to_ndjson(&results);
    assert_eq!(parse_results(&text).unwrap(), results);
    assert!(
        parse_results("{\"id\":\"a\",\"wav\":[0.1]}")
            .unwrap_err()
            .starts_with("wyniki, linia 1:")
    );
}

fn log_line(kind: &str, ts: &str) -> String {
    let kind: EventKind = kind.parse().unwrap();
    let mut v = serde_json::to_value(Event::new(kind, Level::Info, serde_json::json!({}))).unwrap();
    v["ts"] = serde_json::Value::String(ts.into());
    v.to_string()
}

#[test]
fn false_interruptions_from_session_log() {
    let log = [
        log_line("voice.pipeline.pill", "2026-10-01T10:00:00Z"),
        log_line(EVENT_INTERRUPTED, "2026-10-01T10:40:00Z"),
        log_line("voice.pipeline.pill", "2026-10-01T12:00:00Z"),
    ]
    .join("\n");
    assert_eq!(false_interruptions_per_hour(&log), Ok(0.5));
    assert!(false_interruptions_per_hour("").is_err());
    assert!(
        false_interruptions_per_hour(&log_line("voice.pipeline.pill", "2026-10-01T10:00:00Z"))
            .is_err()
    );
    assert!(
        false_interruptions_per_hour("nie json")
            .unwrap_err()
            .contains("linia 1")
    );
}

#[test]
fn schema_and_synthetic_audio() {
    let schema = manifest_schema();
    assert_eq!(schema["title"], "ManifestEntry");
    assert!(schema["required"].as_array().is_some_and(|r| r.len() == 5));
    let spec = SynthSpec {
        seed: 7,
        lead_ms: 300,
        speech_ms: 1_000,
        tail_ms: 700,
    };
    let pcm = audio::synth_audio(&spec);
    assert_eq!(pcm.len(), 2_000 * 16);
    let rms = |s: &[f32]| (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt();
    assert!(rms(&pcm[..4_000]) < 0.01 && rms(&pcm[8_000..16_000]) > 0.1);
    assert_eq!(audio::wav16(&pcm).len(), 44 + pcm.len() * 2);
}
