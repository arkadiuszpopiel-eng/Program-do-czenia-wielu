//! Zamrożony zestaw akceptacyjny voice-cmd (F2): recall na pozytywach ≥ 99 %, odrzucenie
//! negatywów (precision na zestawie negatywnym) ≥ 95 %. Pliki w `tests/data/*.tsv` są zamrożone
//! odciskiem FNV-1a (tu) i SHA-256 (`tests/data/SHA256SUMS`, `sha256sum --check`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use personas_contract::PersonaId;
use voice_cmd_contract::{
    AgentActivity, CmdInput, CmdSource, CommandRecognizer, Token, VoiceCommand,
};
use voice_cmd_impl::GrammarRecognizer;

const POSITIVE: &str = include_str!("data/positive.tsv");
const NEGATIVE: &str = include_str!("data/negative.tsv");
const POSITIVE_FNV: u64 = 0x0e56250de6a29655;
const NEGATIVE_FNV: u64 = 0x3af8db93d2974e37;

fn fnv1a(data: &str) -> u64 {
    data.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn rows(data: &str) -> Vec<Vec<&str>> {
    data.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| l.split('\t').collect())
        .collect()
}

fn activity(phase: &str) -> AgentActivity {
    match phase {
        "speaking" => AgentActivity::Speaking,
        "thinking" => AgentActivity::Thinking,
        "silent" => AgentActivity::Silent,
        other => panic!("nieznana faza {other}"),
    }
}

fn expected(label: &str) -> VoiceCommand {
    match label {
        "stop" => VoiceCommand::Stop,
        "wait" => VoiceCommand::Wait,
        "pause" => VoiceCommand::Pause,
        "resume" => VoiceCommand::Resume,
        "repeat" => VoiceCommand::Repeat,
        "cancel" => VoiceCommand::Cancel,
        "volume_up" => VoiceCommand::VolumeUp,
        "volume_down" => VoiceCommand::VolumeDown,
        "mute_mic" => VoiceCommand::MuteMic,
        "dnd" => VoiceCommand::DoNotDisturb,
        "stop_all" => VoiceCommand::StopAll,
        "no" => VoiceCommand::No,
        other => {
            let id = other.strip_prefix("switch:").expect("etykieta");
            VoiceCommand::SwitchPersona {
                persona: PersonaId::parse(id).unwrap(),
            }
        }
    }
}

/// Syntetyczne znaczniki czasu: słowo 250 ms, przerwa 60 ms, cisza przed (domyślnie 800 ms) i po 800 ms.
fn input(text: &str, phase: &str, pause_before: u64) -> CmdInput {
    let mut t = 10_000;
    let tokens: Vec<Token> = text
        .split_whitespace()
        .map(|w| {
            let tok = Token::new(w, t, t + 250);
            t += 310;
            tok
        })
        .collect();
    let end = tokens.last().map_or(t, |x| x.end_ms);
    CmdInput {
        tokens,
        source: CmdSource::Final,
        activity: activity(phase),
        now_ms: end + 800,
        prev_speech_end_ms: Some(10_000 - pause_before),
        addressed: true,
    }
}

#[test]
fn frozen_files_unchanged() {
    assert_eq!(
        fnv1a(POSITIVE),
        POSITIVE_FNV,
        "zmieniono positive.tsv — wymaga przeglądu człowieka"
    );
    assert_eq!(
        fnv1a(NEGATIVE),
        NEGATIVE_FNV,
        "zmieniono negative.tsv — wymaga przeglądu człowieka"
    );
}

#[test]
fn recall_and_negative_precision() {
    let r = GrammarRecognizer::default();
    let pos = rows(POSITIVE);
    let neg = rows(NEGATIVE);
    assert!(
        pos.len() >= 60 && neg.len() >= 60,
        "za mały zestaw: {} / {}",
        pos.len(),
        neg.len()
    );
    let mut misses = Vec::new();
    let mut wrong = 0usize;
    for row in &pos {
        let decision = r.recognize(&input(row[1], row[0], 800));
        match decision.hit() {
            Some(hit) if hit.command == expected(row[2]) => {}
            Some(_) => {
                wrong += 1;
                misses.push(format!("{} → {decision:?}", row[1]));
            }
            None => misses.push(format!("{} → {decision:?}", row[1])),
        }
    }
    let mut false_hits = Vec::new();
    for row in &neg {
        let pause = row.get(2).map_or(800, |p| p.parse().unwrap());
        let decision = r.recognize(&input(row[1], row[0], pause));
        if decision.hit().is_some() {
            false_hits.push(format!("{} → {decision:?}", row[1]));
        }
    }
    let recall = (pos.len() - misses.len()) as f64 / pos.len() as f64;
    let neg_precision = (neg.len() - false_hits.len()) as f64 / neg.len() as f64;
    let tp = (pos.len() - misses.len()) as f64;
    let precision = tp / (tp + wrong as f64 + false_hits.len() as f64);
    eprintln!(
        "voice-cmd: pozytywy {} recall {:.2}% | negatywy {} odrzucone {:.2}% | precision trafień {:.2}%",
        pos.len(),
        recall * 100.0,
        neg.len(),
        neg_precision * 100.0,
        precision * 100.0
    );
    assert!(recall >= 0.99, "recall {recall:.3}; chybienia: {misses:#?}");
    assert!(
        neg_precision >= 0.95,
        "precision negatywów {neg_precision:.3}; fałszywe: {false_hits:#?}"
    );
}
