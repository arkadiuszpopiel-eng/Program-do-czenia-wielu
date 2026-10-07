//! Planista stylu: biblia głosu + znaczniki → parametry silnika wg tabeli (per silnik).
//!
//! Tabele silników są wartościami wyjściowymi do strojenia w Voice Lab (VOICE.md §15);
//! nazwy stylów chmurowych wymagają weryfikacji z dokumentacją dostawcy przy integracji.

use voice_persona_contract::{
    Emotion, EmotionTag, Energy, EngineKind, EngineStyleTable, ParamRange, SpeechStyle, StylePlan,
    StylePlanner, StyleTags, Tempo, VoiceBible,
};

/// Mnożnik tempa dla znacznika „wolno”.
pub const SLOW_FACTOR: f32 = 0.88;
/// Mnożnik tempa dla znacznika „szybko”.
pub const FAST_FACTOR: f32 = 1.12;

fn tags(pairs: &[(Emotion, &str)]) -> Vec<EmotionTag> {
    pairs
        .iter()
        .map(|(emotion, tag)| EmotionTag {
            emotion: *emotion,
            tag: (*tag).to_owned(),
        })
        .collect()
}

const fn range(min: f32, max: f32) -> Option<ParamRange> {
    Some(ParamRange { min, max })
}

/// Tabela stylu dla silnika.
pub fn table_for(engine: EngineKind) -> EngineStyleTable {
    use Emotion::{Calm, Curiosity, Empathy, Enthusiasm, Joy, Sadness, Serious, Warm};
    let mut t = EngineStyleTable::neutral(engine);
    match engine {
        EngineKind::PocketTts => {
            t.rate = range(0.7, 1.4);
            t.pitch_semitones = range(-4.0, 4.0);
            t.gain_db = range(-6.0, 6.0);
        }
        EngineKind::Piper => {
            t.rate = range(0.7, 1.5);
            t.gain_db = range(-6.0, 6.0);
        }
        EngineKind::ElevenLabs => {
            t.rate = range(0.7, 1.2);
            t.intensity = range(0.0, 1.0);
            t.emotions = tags(&[
                (Warm, "[warmly]"),
                (Joy, "[happy]"),
                (Calm, "[calm]"),
                (Serious, "[serious]"),
                (Empathy, "[gently]"),
                (Enthusiasm, "[excited]"),
                (Curiosity, "[curious]"),
                (Sadness, "[sad]"),
            ]);
        }
        EngineKind::Azure => {
            t.rate = range(0.5, 2.0);
            t.pitch_semitones = range(-12.0, 12.0);
            t.gain_db = range(-10.0, 10.0);
            t.intensity = range(0.01, 2.0);
            t.emotions = tags(&[
                (Warm, "friendly"),
                (Joy, "cheerful"),
                (Calm, "calm"),
                (Serious, "serious"),
                (Empathy, "empathetic"),
                (Enthusiasm, "excited"),
                (Sadness, "sad"),
            ]);
        }
        EngineKind::Cartesia => {
            t.rate = range(0.6, 1.5);
            t.emotions = tags(&[
                (Joy, "positivity"),
                (Enthusiasm, "positivity:high"),
                (Curiosity, "curiosity"),
                (Sadness, "sadness"),
            ]);
        }
        EngineKind::Generic => {}
    }
    t
}

/// Wszystkie tabele wbudowane.
pub fn builtin_tables() -> Vec<EngineStyleTable> {
    [
        EngineKind::PocketTts,
        EngineKind::Piper,
        EngineKind::ElevenLabs,
        EngineKind::Azure,
        EngineKind::Cartesia,
        EngineKind::Generic,
    ]
    .into_iter()
    .map(table_for)
    .collect()
}

/// Polska nazwa emocji (do listy nieobsłużonych znaczników).
pub fn emotion_label(e: Emotion) -> &'static str {
    match e {
        Emotion::Neutral => "neutralnie",
        Emotion::Warm => "ciepło",
        Emotion::Joy => "radość",
        Emotion::Calm => "spokój",
        Emotion::Serious => "powaga",
        Emotion::Empathy => "troska",
        Emotion::Enthusiasm => "entuzjazm",
        Emotion::Curiosity => "ciekawość",
        Emotion::Sadness => "smutek",
    }
}

/// Planista tabelowy (deterministyczny).
#[derive(Debug, Clone, Copy, Default)]
pub struct TablePlanner;

impl StylePlanner for TablePlanner {
    fn plan_style(
        &self,
        bible: &VoiceBible,
        tags: &StyleTags,
        table: &EngineStyleTable,
    ) -> StylePlan {
        let mut unsupported = Vec::new();
        let requested = tags.emotion.unwrap_or(bible.default_emotion);
        let emotion = if bible.emotion_range.contains(&requested) {
            requested
        } else {
            unsupported.push(format!("emocja:{} (poza biblią)", emotion_label(requested)));
            bible.default_emotion
        };
        let mut rate = bible.tempo
            * match tags.tempo {
                Some(Tempo::Slow) => SLOW_FACTOR,
                Some(Tempo::Fast) => FAST_FACTOR,
                _ => 1.0,
            };
        if emotion == Emotion::Serious {
            // Komunikaty o ryzyku bez pośpiechu (PERSONAS §2.4).
            rate = rate.min(1.0);
        }
        let energy = (bible.energy
            + match tags.energy {
                Some(Energy::Low) => -0.2,
                Some(Energy::High) => 0.2,
                _ => 0.0,
            })
        .clamp(0.0, 1.0);
        let pitch = bible.pitch_semitones
            + match emotion {
                Emotion::Joy | Emotion::Enthusiasm => 0.5,
                Emotion::Sadness | Emotion::Serious => -0.5,
                _ => 0.0,
            };
        let mut style = SpeechStyle::neutral(table.engine);
        match table.rate {
            Some(r) => style.rate = r.clamp(rate),
            None if matches!(tags.tempo, Some(Tempo::Slow | Tempo::Fast)) => {
                unsupported.push("tempo".to_owned())
            }
            None => {}
        }
        if let Some(r) = table.pitch_semitones {
            style.pitch_semitones = r.clamp(pitch);
        }
        match table.gain_db {
            Some(r) => style.gain_db = r.clamp((energy - 0.5) * 6.0),
            None if table.intensity.is_none()
                && matches!(tags.energy, Some(Energy::Low | Energy::High)) =>
            {
                unsupported.push("energia".to_owned());
            }
            None => {}
        }
        style.intensity = table.intensity.map(|r| r.clamp(energy));
        style.emotion_tag = table.emotion_tag(emotion).map(str::to_owned);
        if style.emotion_tag.is_none()
            && tags
                .emotion
                .is_some_and(|e| e != Emotion::Neutral && e == emotion)
        {
            unsupported.push(format!("emocja:{}", emotion_label(emotion)));
        }
        StylePlan { style, unsupported }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_persona_contract::PersonaId;

    fn bible(id: PersonaId) -> VoiceBible {
        VoiceBible::builtin(&id).unwrap()
    }

    #[test]
    fn maps_bible_and_tags_per_engine() {
        let alfa = bible(PersonaId::alfa());
        let joy = StyleTags {
            emotion: Some(Emotion::Joy),
            tempo: Some(Tempo::Fast),
            energy: Some(Energy::High),
        };
        let azure = TablePlanner.plan_style(&alfa, &joy, &table_for(EngineKind::Azure));
        assert_eq!(azure.style.emotion_tag.as_deref(), Some("cheerful"));
        assert!((azure.style.rate - FAST_FACTOR).abs() < 1e-6);
        assert!(azure.style.gain_db > 0.0 && azure.unsupported.is_empty());
        let piper = TablePlanner.plan_style(&alfa, &joy, &table_for(EngineKind::Piper));
        assert_eq!(piper.style.emotion_tag, None);
        assert_eq!(piper.unsupported, vec!["emocja:radość".to_owned()]);
        let generic = TablePlanner.plan_style(&alfa, &joy, &table_for(EngineKind::Generic));
        assert_eq!(generic.style, SpeechStyle::neutral(EngineKind::Generic));
        assert_eq!(generic.unsupported.len(), 3);
        assert_eq!(builtin_tables().len(), 6);
    }

    #[test]
    fn serious_caps_tempo_and_range_limits_emotion() {
        let delta = bible(PersonaId::delta());
        let serious = StyleTags {
            emotion: Some(Emotion::Serious),
            ..StyleTags::default()
        };
        let plan = TablePlanner.plan_style(&delta, &serious, &table_for(EngineKind::PocketTts));
        assert!(plan.style.rate <= 1.0);
        let gama = bible(PersonaId::gama());
        let excited = StyleTags {
            emotion: Some(Emotion::Enthusiasm),
            ..StyleTags::default()
        };
        let plan = TablePlanner.plan_style(&gama, &excited, &table_for(EngineKind::ElevenLabs));
        assert_eq!(plan.style.emotion_tag, None);
        assert!(plan.unsupported[0].contains("poza biblią"));
        let slow = StyleTags {
            tempo: Some(Tempo::Slow),
            ..StyleTags::default()
        };
        let plan = TablePlanner.plan_style(&gama, &slow, &table_for(EngineKind::PocketTts));
        assert!(plan.style.rate < gama.tempo && plan.style.pitch_semitones < 0.0);
    }
}
