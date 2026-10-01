//! E2E na atrapach: zmiana agentki w trakcie rozmowy („Gama, …”, „przełącz na Deltę”) — głos
//! (wysokość F0 nagranego wyjścia) i persona (prompt modelu) zmienione bez restartu; PTT —
//! mikrofon otwarty i dzierżawiony tylko w trakcie trzymania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Opts, Timeline, World};
use personas_contract::PersonaId;
use scheduler_lite_contract::{Holder, Resource, SchedulerLite};
use voice_audio_contract::synth::estimate_f0;
use voice_dialog_contract::Command;
use voice_pipeline_contract::{EVENT_PERSONA_SWITCHED, PipelineInput, VoicePipeline};
use voice_wake_contract::MicState;
use voice_wake_contract::contract_tests::KeyDriver;

/// F0 odpowiedzi: okno 400 ms od pierwszej próbki po `from_ms`.
fn reply_f0(w: &World, from_ms: u64, to_ms: u64) -> f32 {
    let start = w
        .first_audio_after(from_ms, to_ms)
        .expect("odpowiedź słyszalna");
    let per = 48usize;
    let seg = w
        .audio
        .recorded_range((start as usize + 100) * per, (start as usize + 500) * per);
    estimate_f0(&seg, 48_000, 120.0, 320.0).expect("F0")
}

#[tokio::test]
async fn persona_switches_live_twenty_times() {
    let mut w = World::new(Opts::default());
    let cycle = [
        ("Gama, jakie mam dziś zadania?", PersonaId::gama(), 184.0),
        ("przełącz na Deltę", PersonaId::delta(), 224.0),
        ("A co jutro?", PersonaId::delta(), 224.0),
        ("Beto, przypomnij mi o zakupach.", PersonaId::beta(), 212.0),
        ("Alfo, co słychać?", PersonaId::alfa(), 200.0),
    ];
    let turns = 25usize;
    let turn_ms = 3_200u64;
    let mut t = Timeline::new(turns as u64 * turn_ms + 2_000, 71);
    for i in 0..turns as u64 {
        t.speech(300 + i * turn_ms, 900, 80 + i);
    }
    w.mic(&t);
    let mut expected = Vec::new();
    for i in 0..turns {
        let (text, persona, f0) = &cycle[i % cycle.len()];
        w.stt.script(*text);
        // Komenda „przełącz na …” nie idzie do modelu — po niej mówi kolejna tura.
        if !text.starts_with("przełącz") {
            w.answer(&["Już się tym zajmuję."]);
        }
        expected.push((persona.clone(), *f0, text.starts_with("przełącz")));
    }
    w.conversation_mode().await;
    let mut checked = 0;
    let mut active = PersonaId::alfa();
    for (i, (persona, f0, command)) in expected.iter().enumerate() {
        let start = 300 + i as u64 * turn_ms;
        w.run_until(start + turn_ms).await;
        assert_eq!(&w.p.status().persona, persona, "tura {i}: aktywna agentka");
        active = persona.clone();
        if *command {
            continue;
        }
        let measured = reply_f0(&w, start + 900, start + turn_ms);
        assert!(
            (measured - f0).abs() / f0 < 0.05,
            "tura {i}: F0 {measured} vs {f0} ({persona})"
        );
        let req = w.provider.requests().last().cloned().unwrap();
        let name = match persona.as_str() {
            "alfa" => "Alfa",
            "beta" => "Beta",
            "gama" => "Gama",
            _ => "Delta",
        };
        assert!(
            req.system.unwrap().contains(name),
            "tura {i}: prompt persony"
        );
        checked += 1;
    }
    let _ = active;
    let switches = w.events(EVENT_PERSONA_SWITCHED);
    eprintln!(
        "zmiana obsady w locie: {} zmian, {checked} odpowiedzi zweryfikowanych głosem (F0) i promptem",
        switches.len()
    );
    assert_eq!(switches.len(), 20, "20/20 zmian bez restartu");
    assert_eq!(
        checked, 20,
        "każda odpowiedź głosem i personą aktywnej agentki"
    );
    assert!(w.trace_commands().iter().any(|(_, c)| matches!(
        c,
        Command::StartTts { persona, .. } if *persona == PersonaId::delta()
    )));
    assert_eq!(
        w.provider.requests().len(),
        20,
        "komendy przełączenia bez LLM"
    );
    w.check_logs();
}

#[tokio::test]
async fn ui_switch_changes_voice_of_next_reply() {
    let mut w = World::new(Opts::default());
    let mut t = Timeline::new(5_000, 91);
    t.speech(500, 900, 92);
    w.mic(&t);
    w.stt.script("Co nowego?");
    w.answer(&["Nic nowego."]);
    w.p.input(PipelineInput::SwitchPersona {
        persona: PersonaId::delta(),
    });
    w.conversation_mode().await;
    w.run_until(4_800).await;
    let f0 = reply_f0(&w, 1_400, 4_800);
    assert!((f0 - 224.0).abs() < 11.0, "{f0}");
}

#[tokio::test]
async fn push_to_talk_opens_mic_only_while_held() {
    let mut w = World::new(Opts::default());
    let mut t = Timeline::new(12_000, 101);
    t.speech(500, 900, 102); // bez PTT — nikt nie słucha
    t.speech(2_200, 900, 103); // z PTT
    t.speech(6_000, 900, 104); // bez PTT
    t.speech(8_200, 1_500, 105); // PTT puszczony w trakcie mowy
    w.mic(&t);
    w.stt.script("Zapisz notatkę o spotkaniu.");
    w.stt.script("Dodaj termin na piątek.");
    w.answer(&["Zapisałam."]);
    w.answer(&["Dodałam termin."]);
    w.run_until(1_900).await;
    let s = w.p.status();
    assert_eq!(
        (s.mic, s.mic_open, w.audio.open_inputs()),
        (MicState::Off, false, 0)
    );
    assert!(w.sched.holder(&Resource::Mic).is_none());
    w.keys.ptt(true);
    w.tick().await;
    assert!(w.p.status().mic_open && w.audio.open_inputs() == 1);
    assert_eq!(w.sched.holder(&Resource::Mic).unwrap().holder, Holder::User);
    w.run_until(3_400).await;
    assert!(matches!(
        w.p.status().mic,
        MicState::Listening | MicState::Hearing
    ));
    w.keys.ptt(false);
    w.tick().await;
    let s = w.p.status();
    assert!(!s.mic_open && w.audio.open_inputs() == 0, "{s:?}");
    assert!(w.sched.holder(&Resource::Mic).is_none());
    w.run_until(7_900).await;
    assert_eq!(
        w.provider.requests().len(),
        1,
        "tylko wypowiedź w trakcie PTT"
    );
    assert!(
        w.first_audio_after(3_400, 6_000).is_some(),
        "odpowiedź po puszczeniu PTT"
    );
    // PTT puszczony w trakcie mowy: tura kończy się od razu (bez cierpliwości).
    w.keys.ptt(true);
    w.run_until(9_000).await;
    w.keys.ptt(false);
    w.tick().await;
    let released = w.now();
    w.run_until(11_800).await;
    let reqs = w.provider.requests();
    assert_eq!(reqs.len(), 2);
    assert_eq!(
        reqs[1].messages.last().unwrap().visible_text(),
        "Dodaj termin na piątek."
    );
    let submit = w
        .first_cmd(released, |c| matches!(c, Command::SubmitTurn { .. }))
        .unwrap();
    assert!(
        submit - released <= 30,
        "tura zamknięta przy puszczeniu: {} ms",
        submit - released
    );
    let states: Vec<String> = w
        .events("voice.wake.mic_state")
        .iter()
        .map(|e| e.payload["state"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(
        states.iter().any(|s| s == "hearing") && states.iter().any(|s| s == "off"),
        "{states:?}"
    );
    assert_eq!(w.stt.pending_script(), 0);
    w.check_logs();
}
