//! Test własności potoku (i): dowolna sekwencja zdarzeń (mowa, PTT, przełącznik, composer, `Esc`,
//! wyjście z trybu głosowego, mowa proaktywna, zmiana agentki, DND, wyciszenie, echo) →
//! po każdym kroku: nigdy dwie agentki naraz w głośniku (dzierżawa = mówiąca), nigdy audio po
//! `StopTts`, mikrofon zawsze w jednym spójnym stanie (strumień = słuchanie = dzierżawa).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{Opts, Timeline, World};
use personas_contract::PersonaId;
use proptest::prelude::*;
use voice_audio_fake::EchoPath;
use voice_pipeline_contract::{PipelineInput, VoicePipeline};
use voice_wake_contract::contract_tests::KeyDriver;

const TEXTS: [&str; 10] = [
    "Jaka jest pogoda?",
    "stop",
    "czekaj",
    "wznów",
    "mhm",
    "Gama, co mam w kalendarzu?",
    "przełącz na Deltę",
    "Chodziło mi o jutro.",
    "powtórz",
    "anuluj",
];

#[derive(Debug, Clone)]
enum Act {
    Ptt(bool),
    Toggle,
    Typed,
    Esc,
    Deactivate,
    Proactive(u8),
    Switch(u8),
    Dnd(bool),
    Mute(bool),
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        3 => any::<bool>().prop_map(Act::Ptt),
        3 => Just(Act::Toggle),
        1 => Just(Act::Typed),
        1 => Just(Act::Esc),
        1 => Just(Act::Deactivate),
        1 => (0u8..4).prop_map(Act::Proactive),
        1 => (0u8..4).prop_map(Act::Switch),
        1 => any::<bool>().prop_map(Act::Dnd),
        1 => any::<bool>().prop_map(Act::Mute),
    ]
}

fn persona(i: u8) -> PersonaId {
    PersonaId::builtin()[usize::from(i % 4)].clone()
}

#[derive(Debug, Clone)]
struct Scenario {
    echo: bool,
    speech: Vec<(u64, u64, usize)>,
    acts: Vec<(u64, Act)>,
}

fn scenario() -> impl Strategy<Value = Scenario> {
    (
        any::<bool>(),
        prop::collection::vec((0u64..9_000, 250u64..1_800, 0usize..TEXTS.len()), 1..8),
        prop::collection::vec((0u64..9_500, act()), 1..14),
    )
        .prop_map(|(echo, mut speech, mut acts)| {
            speech.sort_by_key(|s| s.0);
            acts.sort_by_key(|a| a.0);
            Scenario { echo, speech, acts }
        })
}

async fn run(sc: Scenario) {
    let mut w = World::new(Opts {
        echo: sc
            .echo
            .then(|| EchoPath::sparse_room(Duration::from_millis(25), 0.3, 120, 48_000, 3, 30)),
        ..Opts::default()
    });
    let mut t = Timeline::new(11_000, 7);
    for (i, (at, ms, text)) in sc.speech.iter().enumerate() {
        t.speech(*at, *ms, i as u64 + 1);
        w.stt.script(TEXTS[*text]);
        w.answer(&["Pierwsze zdanie odpowiedzi.", "Drugie zdanie odpowiedzi."]);
    }
    w.mic(&t);
    w.p.input(PipelineInput::Toggle);
    let mut acts = sc.acts.into_iter().peekable();
    while w.now() < 10_500 {
        while let Some((_, a)) = acts.next_if(|(at, _)| *at <= w.now()) {
            match a {
                Act::Ptt(p) => w.keys.ptt(p),
                Act::Toggle => w.p.input(PipelineInput::Toggle),
                Act::Typed => w.p.input(PipelineInput::Typed {
                    text: "napisane pytanie".into(),
                }),
                Act::Esc => w.p.input(PipelineInput::StopSpeech),
                Act::Deactivate => w.p.input(PipelineInput::Deactivate),
                Act::Proactive(p) => w.p.input(PipelineInput::Proactive {
                    persona: persona(p),
                    text: "Przypominam o spotkaniu.".into(),
                    reason: "przypomnienie".into(),
                }),
                Act::Switch(p) => w.p.input(PipelineInput::SwitchPersona {
                    persona: persona(p),
                }),
                Act::Dnd(on) => w.p.input(PipelineInput::SetDoNotDisturb { on }),
                Act::Mute(m) => w.p.input(PipelineInput::SetMuted { muted: m }),
            }
        }
        // `tick` sprawdza niezmienniki mikrofonu i głośnika po każdym kroku.
        w.tick().await;
    }
    w.check_logs();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(40))]

    #[test]
    fn any_event_sequence_keeps_speaker_and_mic_invariants(sc in scenario()) {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(run(sc));
    }
}
