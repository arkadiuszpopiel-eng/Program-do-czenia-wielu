//! Migawka potoku → pigułka głosowa i wskaźnik mikrofonu w UI (kto mówi, poziom, transkrypt
//! częściowy, stan mikrofonu — nigdy sam kolor).

use app_api::dto::{MicState, VoicePillState, VoiceSpeaker};
use voice_dialog_contract::DialogPhase;
use voice_pipeline_contract::{PipelineStatus, Speaker};

/// Poziom 0..1 z dBFS (−60…0 dB).
pub fn level(db: f32) -> f64 {
    ((f64::from(db) + 60.0) / 60.0).clamp(0.0, 1.0)
}

/// Stan mikrofonu w UI.
pub fn mic_state(status: &PipelineStatus) -> MicState {
    if status.phase == DialogPhase::Speaking {
        return MicState::Speaking;
    }
    match status.mic {
        voice_wake_contract::MicState::Off => MicState::Off,
        voice_wake_contract::MicState::Listening => MicState::Listening,
        voice_wake_contract::MicState::Hearing => MicState::Hearing,
        voice_wake_contract::MicState::Processing => MicState::Processing,
        voice_wake_contract::MicState::Muted => MicState::Muted,
    }
}

/// Pigułka z migawki.
pub fn pill(status: &PipelineStatus) -> VoicePillState {
    let (speaker, agent) = match &status.speaker {
        Speaker::Nobody => (VoiceSpeaker::Nobody, status.persona.as_str().to_owned()),
        Speaker::User => (VoiceSpeaker::User, status.persona.as_str().to_owned()),
        Speaker::Agent(p) => (VoiceSpeaker::Agent, p.as_str().to_owned()),
    };
    let partial = status.partial.trim();
    VoicePillState {
        agent,
        mic: mic_state(status),
        level: level(status.level_db),
        speaker,
        partial: (!partial.is_empty()).then(|| partial.to_owned()),
    }
}

/// Czy pigułka zmieniła się istotnie (poza samym poziomem).
pub fn changed(a: &VoicePillState, b: &VoicePillState) -> bool {
    a.agent != b.agent || a.mic != b.mic || a.speaker != b.speaker || a.partial != b.partial
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_and_changes() {
        assert!(level(-90.0).abs() < f64::EPSILON);
        assert!((level(0.0) - 1.0).abs() < f64::EPSILON);
        let a = VoicePillState {
            agent: "alfa".into(),
            mic: MicState::Listening,
            level: 0.1,
            speaker: VoiceSpeaker::Nobody,
            partial: None,
        };
        let mut b = a.clone();
        b.level = 0.9;
        assert!(!changed(&a, &b));
        b.partial = Some("co mam".into());
        assert!(changed(&a, &b));
    }
}
