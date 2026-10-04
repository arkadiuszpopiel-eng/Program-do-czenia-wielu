//! Specyfikacje komend głosu rozszerzonego F5 (`voice_features`, `voice_wake`, `voice_speaker`,
//! `voice_dictation`, `voice_read`) dla `dto_roundtrip.rs`.

use app_core::dto::*;

use crate::{Check, roundtrip};

use super::Spec;

/// Specyfikacja komendy głosu F5 (`None` — komenda spoza tej części).
pub fn spec(command: &str) -> Option<Spec> {
    let view: Check = roundtrip::<VoiceFeatures>;
    Some(match command {
        "voice_features" => (vec![], view),
        "voice_wake" => (vec![("action", roundtrip::<WakeAction>)], view),
        "voice_speaker" => (vec![("action", roundtrip::<SpeakerAction>)], view),
        "voice_dictation" => (vec![("action", roundtrip::<DictationAction>)], view),
        "voice_read" => (vec![("action", roundtrip::<ReadAction>)], view),
        _ => return None,
    })
}
