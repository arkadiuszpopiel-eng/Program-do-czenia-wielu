//! Metryki §6.4 / F2 na scenariuszach syntetycznych (wirtualny zegar, deterministyczny LCG):
//! twardy stop ≤ 400 ms od początku mowy (ducking w tym samym kroku), fałszywe przerwania = 0
//! przy backchannelu co 3 s przez 60 s, prefiks ±1 słowo ze znacznikami ≥ 90 %.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Lcg, Sim};
use voice_dialog_contract::{Command, DialogEvent, DialogNotice, UtteranceId};
use voice_dialog_impl::default_machine;

/// Przebieg osi czasu: zdarzenia użytkownika + postęp odtwarzania co 20 ms + tyknięcia co 10 ms.
fn run<A: voice_dialog_contract::DialogAutomaton>(
    sim: &mut Sim<A>,
    id: UtteranceId,
    audio_start: u64,
    latency: u64,
    mut events: Vec<(u64, DialogEvent)>,
    until: u64,
) {
    events.sort_by_key(|(t, _)| *t);
    let mut next = 0;
    let mut t = sim.now;
    while t <= until {
        while next < events.len() && events[next].0 <= t {
            let (at, e) = events[next].clone();
            sim.at(at, e);
            next += 1;
        }
        if t >= audio_start && (t - audio_start).is_multiple_of(20) {
            let played = (t - audio_start + latency) * 48;
            sim.at(
                t,
                DialogEvent::PlaybackProgress {
                    utterance: id,
                    played_samples: played,
                    sample_rate: 48_000,
                    device_latency_ms: latency,
                },
            );
        } else {
            sim.at(t, DialogEvent::Tick);
        }
        t += 10;
    }
}

fn words(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("słowo{i}")).collect()
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[test]
fn hard_stop_within_400ms_and_duck_immediately() {
    let contents = [
        "nie, chodziło mi o jutro",
        "a co z pocztą",
        "czekaj, zmień to",
        "zrób to inaczej",
        "tak, ale nie tak",
    ];
    let mut rng = Lcg(42);
    let mut latencies = Vec::new();
    for i in 0..120 {
        let mut sim = Sim::new(default_machine());
        let id = sim.speak(1000, "Opowiedz mi o pogodzie.");
        let w = words(60);
        let refs: Vec<&str> = w.iter().map(String::as_str).collect();
        sim.queue_words(id, &refs, 300, 50, true);
        let b = 2000 + rng.range(300, 12_000) / 10 * 10;
        let mut events = vec![(b, DialogEvent::VadSpeechStart)];
        // Co piąty scenariusz: STT bez transkryptu w oknie potwierdzenia.
        if i % 5 != 0 {
            let text = contents[i % contents.len()].to_owned();
            events.push((
                b + rng.range(80, 320) / 10 * 10,
                DialogEvent::UserPartial { text },
            ));
        }
        run(&mut sim, id, 1900, 30, events, b + 700);
        let duck = sim
            .first(|c| matches!(c, Command::DuckOutput { .. }))
            .unwrap();
        assert_eq!(duck, b, "ducking w tym samym kroku co VAD (< 50 ms)");
        let stop = sim
            .first(|c| *c == Command::StopTts { utterance: id })
            .expect("twardy stop");
        assert!(sim.count(|c| *c == Command::CancelGeneration) == 1);
        latencies.push(stop - b);
    }
    latencies.sort_unstable();
    let max = *latencies.last().unwrap();
    eprintln!(
        "twardy stop od początku mowy: p50 {} ms, p95 {} ms, max {} ms (n = {})",
        percentile(&latencies, 0.5),
        percentile(&latencies, 0.95),
        max,
        latencies.len()
    );
    assert!(max <= 400, "twardy stop po {max} ms");
}

#[test]
fn backchannel_every_3s_for_60s_gives_zero_false_interruptions() {
    let phrases: [(&[&str], u64); 7] = [
        (&["mhm"], 300),
        (&["tak"], 250),
        (&["aha"], 280),
        (&["okej"], 350),
        (&["nie", "nie no", "nie no, dobrze"], 750),
        (&["no", "no tak"], 450),
        (&["jasne"], 330),
    ];
    let mut sim = Sim::new(default_machine());
    let id = sim.speak(1000, "Przeczytaj mi długi artykuł.");
    let w = words(190);
    let refs: Vec<&str> = w.iter().map(String::as_str).collect();
    sim.queue_words(id, &refs, 280, 40, true);
    let mut rng = Lcg(7);
    let mut events = Vec::new();
    let mut count = 0;
    let mut t = 3000;
    while t < 3000 + 60_000 {
        let (parts, dur) = phrases[count % phrases.len()];
        events.push((t, DialogEvent::VadSpeechStart));
        let step = dur / (parts.len() as u64 + 1);
        for (k, p) in parts.iter().enumerate() {
            let at = t + rng
                .range(100, 160)
                .min(step * (k as u64 + 1))
                .max(step * (k as u64 + 1) - 20);
            events.push((
                at,
                DialogEvent::UserPartial {
                    text: (*p).to_owned(),
                },
            ));
        }
        events.push((t + dur, DialogEvent::VadSpeechEnd));
        count += 1;
        t += 3000;
    }
    run(&mut sim, id, 1900, 30, events, 3000 + 60_000);
    let stops = sim.count(|c| matches!(c, Command::StopTts { .. }));
    let backchannels = sim.count(|c| {
        matches!(
            c,
            Command::Notify {
                notice: DialogNotice::Backchannel { .. }
            }
        )
    });
    let ducks = sim.count(|c| matches!(c, Command::DuckOutput { .. }));
    let restores = sim.count(|c| *c == Command::RestoreOutput);
    eprintln!(
        "backchannel co 3 s / 60 s: {count} wypowiedzi, fałszywe przerwania {stops}, rozpoznane backchannele {backchannels}, ducking {ducks}/{restores}"
    );
    assert_eq!(count, 20);
    assert_eq!(stops, 0, "fałszywe przerwania");
    assert_eq!(sim.count(|c| *c == Command::CancelGeneration), 0);
    assert_eq!(backchannels, 20);
    assert_eq!((ducks, restores), (20, 20));
}

/// Prawda: słowa, które skończyły się na urządzeniu przed twardym stopem.
fn truth(marks: &[voice_dialog_contract::WordMark], device_pos: u64) -> usize {
    marks.iter().filter(|m| m.end_ms <= device_pos).count()
}

fn prefix_scenarios(with_marks: bool) -> (usize, usize) {
    let mut rng = Lcg(2026);
    let (mut ok, mut total) = (0, 0);
    for _ in 0..120 {
        let mut sim = Sim::new(default_machine());
        let id = sim.speak(1000, "Opowiedz coś.");
        let n = rng.range(20, 40) as usize;
        let w: Vec<String> = (0..n)
            .map(|i| "x".repeat(rng.range(2, 11) as usize) + &i.to_string())
            .collect();
        let refs: Vec<&str> = w.iter().map(String::as_str).collect();
        let word_ms = rng.range(180, 420);
        let (_, marks) = sim.queue_words(id, &refs, word_ms, rng.range(30, 120), with_marks);
        let audio_end = marks.last().unwrap().end_ms;
        let latency = rng.range(10, 60);
        let b = 1900 + rng.range(200, audio_end - 100) / 10 * 10;
        let events = vec![
            (b, DialogEvent::VadSpeechStart),
            (
                b + 150,
                DialogEvent::UserPartial {
                    text: "nie, zaczekaj".into(),
                },
            ),
        ];
        run(&mut sim, id, 1900, latency, events, b + 450);
        let stop = sim
            .first(|c| *c == Command::StopTts { utterance: id })
            .unwrap();
        let heard = sim
            .log
            .iter()
            .find_map(|(_, c)| match c {
                Command::Notify {
                    notice: DialogNotice::Interrupted { heard },
                } => Some(heard.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(heard.approximate, !with_marks);
        let expected = truth(&marks, stop - 1900);
        total += 1;
        if heard.words.abs_diff(expected) <= 1 {
            ok += 1;
        }
    }
    (ok, total)
}

#[test]
fn heard_prefix_accuracy() {
    let (ok, total) = prefix_scenarios(true);
    let (ok_approx, total_approx) = prefix_scenarios(false);
    eprintln!(
        "prefiks ±1 słowo: znaczniki TTS {ok}/{total} ({:.1} %), liczenie próbek (approximate) {ok_approx}/{total_approx} ({:.1} %)",
        100.0 * ok as f64 / total as f64,
        100.0 * ok_approx as f64 / total_approx as f64
    );
    assert!(
        ok * 100 >= total * 90,
        "prefiks ze znacznikami poniżej 90 %"
    );
}
