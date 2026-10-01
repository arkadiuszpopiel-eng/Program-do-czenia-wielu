//! E2E na atrapach: barge-in (ducking natychmiast, twardy stop ≤ 400 ms, usłyszany prefiks
//! w historii, odpowiedź na korektę), backchannel co 3 s przez 60 s bez przerwań, „stop”/„czekaj”
//! < 300 ms (+ „wznów” od punktu cięcia), echo własnego TTS przez „pokój” bez przerwania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{Opts, Timeline, World};
use voice_audio_fake::EchoPath;
use voice_dialog_contract::{Command, DialogNotice, DialogPhase, InterruptIntent};
use voice_pipeline_contract::{EchoGateCfg, PipelineCfg, VoicePipeline};
use voice_tts_contract::{Tts, TtsRequest};

fn room() -> Option<EchoPath> {
    Some(EchoPath::sparse_room(
        Duration::from_millis(30),
        0.25,
        150,
        48_000,
        5,
        40,
    ))
}

fn long_reply() -> Vec<&'static str> {
    vec![
        "Jutro rano masz spotkanie zespołu o dziewiątej.",
        "Potem przegląd budżetu z działem finansów.",
        "Po południu rozmowa z klientem o nowej umowie.",
        "Wieczorem przypomnienie o urodzinach siostry.",
    ]
}

fn count(w: &World, from: u64, pred: impl Fn(&Command) -> bool) -> usize {
    w.p.trace()
        .into_iter()
        .filter(|e| e.at_ms >= from && pred(&e.command))
        .count()
}

/// Słowa mówione usłyszane do `played_ms` wypowiedzi (znaczniki atrapy TTS, te same zdania).
async fn true_heard_words(lines: &[&str], played_ms: u64) -> usize {
    let tts = voice_tts_fake::FakeTts::new();
    let (mut offset, mut words) = (0u64, 0usize);
    for (i, line) in lines.iter().enumerate() {
        let req = TtsRequest {
            utterance: 1,
            persona: personas_contract::PersonaId::alfa(),
            text: (*line).to_owned(),
            style: Default::default(),
            cacheable: false,
            privacy: Default::default(),
        };
        let mut rx = tts.synth(req, Default::default()).await.unwrap();
        let chunk = rx.recv().await.unwrap().unwrap();
        for m in &chunk.marks {
            if offset + u64::from(m.end_ms) <= played_ms {
                words += 1;
            }
        }
        offset += u64::try_from(chunk.audio.duration().as_millis()).unwrap();
        let _ = i;
    }
    words
}

async fn barge_in(echo: bool) {
    let mut w = World::new(Opts {
        echo: if echo { room() } else { None },
        ..Opts::default()
    });
    let mut t = Timeline::new(16_000, 21);
    t.speech(300, 1_200, 22);
    t.speech(6_000, 1_500, 23);
    w.mic(&t);
    w.stt.script("Co mam jutro w planie?");
    w.stt.script("Chodziło mi o wtorek, nie o środę.");
    let lines = long_reply();
    w.answer(&lines);
    w.answer(&["Rozumiem, we wtorek masz tylko jedno spotkanie."]);
    w.conversation_mode().await;
    w.run_until(14_000).await;
    let first_audio = {
        let l = w.p.trace();
        let _ = l;
        w.first_audio_after(1_500, 6_000).unwrap()
    };
    let duck = w
        .first_cmd(6_000, |c| matches!(c, Command::DuckOutput { .. }))
        .expect("ducking");
    let stop = w
        .first_cmd(6_000, |c| matches!(c, Command::StopTts { .. }))
        .expect("twardy stop");
    eprintln!(
        "barge-in (echo: {echo}): ducking {} ms, twardy stop {} ms od początku mowy",
        duck - 6_000,
        stop - 6_000
    );
    // Ducking w tym samym kroku co detekcja VAD; z echem detekcja startu mowy jest ostrożniejsza
    // (atrapa DSP nie tłumi echa — ERLE 0 — więc działa sama reguła echa).
    assert!(
        duck - 6_000 <= if echo { 100 } else { 50 },
        "ducking {} ms",
        duck - 6_000
    );
    assert!(stop - 6_000 <= 400, "twardy stop {} ms", stop - 6_000);
    // Po stopie cisza (rampa 5 ms + opóźnienie wyjścia), aż do odpowiedzi na korektę.
    assert!(
        w.output_rms(stop + 40, stop + 600) < 1e-4,
        "audio po StopTts"
    );
    let heard = w.p.status().heard_prefix.expect("usłyszany prefiks");
    let spoken = lines.join(" ");
    assert!(
        !heard.text.is_empty() && spoken.starts_with(&heard.text),
        "{heard:?}"
    );
    assert!(!heard.approximate, "znaczniki słów z TTS");
    let truth = true_heard_words(&lines, stop - first_audio).await;
    assert!(
        heard.words.abs_diff(truth) <= 1,
        "prefiks {} vs {truth} słów",
        heard.words
    );
    // Historia: pełna tura + notka o usłyszanym prefiksie, potem korekta.
    let reqs = w.provider.requests();
    assert_eq!(reqs.len(), 2);
    let msgs = &reqs[1].messages;
    let note = &msgs[msgs.len() - 2];
    assert!(
        note.visible_text().contains(&format!("„{}”", heard.text)),
        "{}",
        note.visible_text()
    );
    assert_eq!(
        msgs.last().unwrap().visible_text(),
        "Chodziło mi o wtorek, nie o środę."
    );
    let history = w.reply.history();
    assert_eq!(
        history[1].interruption.as_ref().unwrap().heard_prefix,
        heard.text
    );
    assert_eq!(history[1].visible_text(), lines.join("\n") + "\n");
    // Intencja „korekta” i odpowiedź na korektę słyszalna.
    assert!(w.p.trace().iter().any(|e| matches!(
        &e.command,
        Command::Notify {
            notice: DialogNotice::IntentClassified {
                intent: InterruptIntent::Correction,
                ..
            }
        }
    )));
    assert!(
        w.first_audio_after(7_500, 14_000).is_some(),
        "odpowiedź na korektę"
    );
    assert_eq!(w.p.status().interruptions, 1);
    w.check_logs();
}

#[tokio::test]
async fn barge_in_ducks_stops_and_keeps_heard_prefix() {
    barge_in(false).await;
}

#[tokio::test]
async fn barge_in_works_through_room_echo() {
    barge_in(true).await;
}

#[tokio::test]
async fn backchannel_every_3s_for_60s_never_interrupts() {
    let mut w = World::new(Opts {
        echo: room(),
        ..Opts::default()
    });
    let mut t = Timeline::new(72_000, 31);
    t.speech(300, 1_000, 32);
    let words = ["mhm", "aha", "tak", "mhm", "okej"];
    w.stt.script("Opowiedz mi o projekcie.");
    for i in 0..20u64 {
        t.speech(4_000 + 3_000 * i, 350, 40 + i);
        w.stt.script(words[(i % 5) as usize]);
    }
    w.mic(&t);
    let lines: Vec<String> = (0..24)
        .map(|i| {
            format!(
                "To jest zdanie numer {} opisu projektu, dość długie.",
                i + 1
            )
        })
        .collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    w.answer(&refs);
    w.conversation_mode().await;
    w.run_until(64_500).await;
    assert_eq!(
        w.p.dialog_state().phase,
        DialogPhase::Speaking,
        "agentka mówi dalej"
    );
    let stops = count(&w, 0, |c| matches!(c, Command::StopTts { .. }));
    let backchannels = count(&w, 0, |c| {
        matches!(
            c,
            Command::Notify {
                notice: DialogNotice::Backchannel { .. }
            }
        )
    });
    let ducks = count(&w, 0, |c| matches!(c, Command::DuckOutput { .. }));
    eprintln!(
        "backchannel 60 s: {backchannels}/20 rozpoznanych, {ducks} duckingów, {stops} przerwań"
    );
    assert_eq!(stops, 0);
    assert_eq!(w.p.status().interruptions, 0);
    assert_eq!(backchannels, 20);
    assert_eq!(
        w.provider.requests().len(),
        1,
        "backchannel nie trafia do modelu"
    );
    w.check_logs();
}

async fn command_reaction(word: &str) -> (World, u64) {
    let mut w = World::new(Opts::default());
    let mut t = Timeline::new(12_000, 51);
    t.speech(300, 1_200, 52);
    t.speech(5_000, 300, 53);
    t.speech(8_000, 450, 54);
    w.mic(&t);
    w.stt.script("Co mam jutro w planie?");
    w.stt.script(word);
    w.stt.script("wznów");
    w.answer(&long_reply());
    w.conversation_mode().await;
    w.run_until(5_600).await;
    let stop = w
        .first_cmd(5_000, |c| matches!(c, Command::StopTts { .. }))
        .expect("stop mowy");
    (w, stop - 5_000)
}

#[tokio::test]
async fn stop_and_wait_commands_react_under_300ms() {
    let (mut stop_w, stop_ms) = command_reaction("stop").await;
    assert!(
        stop_w.output_rms(5_000 + stop_ms + 40, 5_600) < 1e-4,
        "cisza po „stop”"
    );
    assert!(!stop_w.events(voice_cmd_contract::EVENT_DETECTED).is_empty());
    assert_eq!(
        count(&stop_w, 5_000, |c| matches!(c, Command::CancelGeneration)),
        1
    );
    stop_w.run_until(7_900).await;
    assert_eq!(
        stop_w.provider.requests().len(),
        1,
        "„stop” nie trafia do modelu"
    );
    assert_eq!(stop_w.p.dialog_state().phase, DialogPhase::Listening);
    let (mut wait_w, wait_ms) = command_reaction("czekaj").await;
    eprintln!("reakcja na komendy (zegar wirtualny): „stop” {stop_ms} ms, „czekaj” {wait_ms} ms");
    assert!(stop_ms < 300 && wait_ms < 300, "{stop_ms} / {wait_ms}");
    // „Czekaj” zatrzymuje mowę bez anulowania generowania; „wznów” mówi dalej od punktu cięcia.
    assert_eq!(
        count(&wait_w, 5_000, |c| matches!(c, Command::CancelGeneration)),
        0
    );
    wait_w.run_until(11_900).await;
    assert!(
        wait_w
            .p
            .trace()
            .iter()
            .any(|e| matches!(e.command, Command::ResumeFrom { .. }))
    );
    assert!(
        wait_w.first_audio_after(8_400, 11_900).is_some(),
        "mowa wznowiona"
    );
    assert_eq!(wait_w.provider.requests().len(), 1, "komendy bez LLM");
    stop_w.check_logs();
    wait_w.check_logs();
}

fn echo_world(rule: bool) -> World {
    let cfg = PipelineCfg {
        echo: EchoGateCfg {
            enabled: rule,
            ..EchoGateCfg::default()
        },
        trace_capacity: 100_000,
        ..PipelineCfg::default()
    };
    World::new(Opts {
        cfg,
        echo: room(),
        ..Opts::default()
    })
}

#[tokio::test]
async fn own_tts_echo_through_room_does_not_interrupt() {
    for rule in [true, false] {
        let mut w = echo_world(rule);
        let mut t = Timeline::new(14_000, 61);
        t.speech(300, 1_200, 62);
        w.mic(&t);
        w.stt.script("Co mam jutro w planie?");
        w.answer(&long_reply());
        w.conversation_mode().await;
        w.run_until(13_500).await;
        let ducks = count(&w, 1_600, |c| matches!(c, Command::DuckOutput { .. }));
        let stops = count(&w, 1_600, |c| matches!(c, Command::StopTts { .. }));
        let gated = w.p.status().echo_gated_frames;
        eprintln!(
            "echo pokoju (reguła {rule}): {ducks} duckingów, {stops} przerwań, {gated} ramek echa, sprzężenie {:.1} dB",
            w.p.echo_coupling_db()
        );
        if rule {
            assert_eq!((ducks, stops), (0, 0));
            assert!(gated > 100);
            assert_eq!(
                w.p.dialog_state().phase,
                DialogPhase::Listening,
                "odpowiedź do końca"
            );
            assert!(w.reply.history()[1].interruption.is_none());
        } else {
            assert!(ducks > 0, "kontrola: bez reguły echo wywołuje barge-in");
        }
        w.check_logs();
    }
}
