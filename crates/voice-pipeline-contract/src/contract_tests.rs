//! Współdzielone testy kontraktowe `VoicePipeline` (feature `contract-tests`) — uruchamiane na
//! `voice-pipeline-fake` i na `voice-pipeline-impl` złożonym z atrap. Potok musi być świeży
//! (faza `Idle`, mikrofon wyłączony, aktywna Alfa).

use personas_contract::PersonaId;
use voice_dialog_contract::DialogPhase;
use voice_wake_contract::MicState;

use crate::{PipelineInput, VoicePipeline};

async fn steps(p: &mut dyn VoicePipeline, n: usize) {
    for _ in 0..n {
        p.step().await;
    }
}

/// Stan początkowy i przełącznik mikrofonu: otwarty tylko w trybie słuchania, jeden stan naraz.
pub async fn toggle_opens_and_closes_mic(p: &mut dyn VoicePipeline) {
    let s = p.status();
    assert_eq!(
        (s.phase, s.mic, s.mic_open),
        (DialogPhase::Idle, MicState::Off, false)
    );
    p.input(PipelineInput::Toggle);
    steps(p, 2).await;
    let s = p.status();
    assert!(s.mic_open, "mikrofon otwarty po przełączniku");
    assert!(matches!(s.mic, MicState::Listening | MicState::Hearing));
    assert_eq!(s.phase, DialogPhase::Listening);
    p.input(PipelineInput::Toggle);
    steps(p, 2).await;
    let s = p.status();
    assert!(!s.mic_open && s.mic == MicState::Off, "{s:?}");
}

/// Wyciszenie blokuje słuchanie; po odciszeniu mikrofon pozostaje wyłączony.
pub async fn mute_blocks_listening(p: &mut dyn VoicePipeline) {
    p.input(PipelineInput::SetMuted { muted: true });
    p.input(PipelineInput::Toggle);
    steps(p, 2).await;
    let s = p.status();
    assert_eq!((s.mic, s.mic_open), (MicState::Muted, false));
    p.input(PipelineInput::SetMuted { muted: false });
    steps(p, 2).await;
    let s = p.status();
    assert_eq!((s.mic, s.mic_open), (MicState::Off, false));
}

/// Zmiana agentki w locie (bez restartu) i czas płynący krokami.
pub async fn persona_switch_is_live(p: &mut dyn VoicePipeline) {
    let t0 = p.status().now_ms;
    p.input(PipelineInput::SwitchPersona {
        persona: PersonaId::gama(),
    });
    steps(p, 3).await;
    let s = p.status();
    assert_eq!(s.persona, PersonaId::gama());
    assert!(s.now_ms > t0, "czas potoku płynie ({t0} → {})", s.now_ms);
    p.input(PipelineInput::SwitchPersona {
        persona: PersonaId::alfa(),
    });
    steps(p, 1).await;
    assert_eq!(p.status().persona, PersonaId::alfa());
}

/// Cały zestaw na jednej instancji.
pub async fn run_all(p: &mut dyn VoicePipeline) {
    toggle_opens_and_closes_mic(p).await;
    mute_blocks_listening(p).await;
    persona_switch_is_live(p).await;
}
