//! Biblia głosu per agentka (docs/PERSONAS.md §2, PLAN §6.6).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Emotion, PersonaError, PersonaId};

/// Dolna granica postrzeganego wieku głosu (młoda dorosła, zatwierdzone).
pub const MIN_AGE: u8 = 18;
/// Górna granica postrzeganego wieku głosu.
pub const MAX_AGE: u8 = 25;
/// Słowa zakazane w prompcie voice design (PLAN §6.6).
pub const FORBIDDEN_PROMPT_WORDS: [&str; 3] = ["girl", "cute", "child"];

/// Rejestr głosu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Register {
    /// Niski (Gama).
    Low,
    /// Środkowy (Alfa).
    Middle,
    /// Środkowo-wysoki (Beta, Delta).
    MiddleHigh,
}

/// Pochodzenie głosu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    /// Głos v0 wbudowany w silnik (bez kluczy; wysokość + tempo).
    BuiltinV0,
    /// Głos zaprojektowany z opisu tekstowego (voice design w chmurze).
    VoiceDesign,
    /// Lokalny klon z referencji pochodzącej z voice design (nigdy z prawdziwej osoby).
    CloneFromDesign {
        /// Identyfikator referencji audio (artefakt, nie ścieżka z danymi osobowymi).
        reference: String,
    },
}

/// Zapis zgody i pochodzenia (brak klonów prawdziwych osób).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Consent {
    /// Czy głos pochodzi od prawdziwej osoby — musi być `false`.
    pub real_person: bool,
    /// Notatka o pochodzeniu.
    pub note: String,
}

/// Biblia głosu persony.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VoiceBible {
    /// Persona.
    pub persona: PersonaId,
    /// Imię wyświetlane.
    pub display_name: String,
    /// Język (BCP 47), np. `pl-PL`.
    pub lang: String,
    /// Postrzegany wiek (18–25).
    pub perceived_age: u8,
    /// Barwa (opis).
    pub timbre: String,
    /// Rejestr.
    pub register: Register,
    /// Tempo jako mnożnik tempa naturalnego (1.0 = naturalne).
    pub tempo: f32,
    /// Energia 0.0–1.0 (0.5 = neutralna).
    pub energy: f32,
    /// Przesunięcie wysokości w półtonach względem głosu bazowego.
    pub pitch_semitones: f32,
    /// Emocja domyślna.
    pub default_emotion: Emotion,
    /// Dozwolony zakres emocji; emocje spoza zakresu są zastępowane domyślną.
    pub emotion_range: Vec<Emotion>,
    /// Prompt voice design (bez słów „girl/cute/child”).
    pub voice_prompt: String,
    /// Pochodzenie głosu.
    pub provenance: Provenance,
    /// Zgoda / pochodzenie.
    pub consent: Consent,
}

impl VoiceBible {
    /// Sprawdza niezmienniki biblii (SPEC: wiek 18–25, brak zakazanych słów, brak prawdziwej osoby).
    pub fn validate(&self) -> Result<(), PersonaError> {
        let invalid = |reason: String| Err(PersonaError::InvalidBible { reason });
        if !(MIN_AGE..=MAX_AGE).contains(&self.perceived_age) {
            return invalid(format!(
                "wiek {} poza zakresem {MIN_AGE}–{MAX_AGE}",
                self.perceived_age
            ));
        }
        let prompt = self.voice_prompt.to_lowercase();
        if let Some(word) = FORBIDDEN_PROMPT_WORDS.iter().find(|w| prompt.contains(*w)) {
            return invalid(format!("zakazane słowo w prompcie: {word}"));
        }
        if self.consent.real_person {
            return invalid("klonowanie prawdziwych osób jest zabronione".into());
        }
        let finite = [self.tempo, self.energy, self.pitch_semitones]
            .iter()
            .all(|v| v.is_finite());
        if !finite || self.tempo <= 0.0 || !(0.0..=1.0).contains(&self.energy) {
            return invalid("tempo > 0 i energia 0–1 są wymagane".into());
        }
        Ok(())
    }

    /// Biblia wbudowanej persony (wartości z docs/PERSONAS.md §2, do strojenia w Voice Lab).
    pub fn builtin(persona: &PersonaId) -> Option<Self> {
        Self::builtin_all()
            .into_iter()
            .find(|b| &b.persona == persona)
    }

    /// Biblie czterech person wbudowanych.
    pub fn builtin_all() -> Vec<Self> {
        vec![
            seed(Seed {
                persona: PersonaId::alfa(),
                name: "Alfa",
                age: 23,
                timbre: "ciepła, środkowy rejestr",
                register: Register::Middle,
                tempo: 1.0,
                energy: 0.45,
                pitch: 0.0,
                emotion: Emotion::Warm,
                range: &[
                    Emotion::Neutral,
                    Emotion::Warm,
                    Emotion::Calm,
                    Emotion::Joy,
                    Emotion::Serious,
                    Emotion::Empathy,
                ],
                prompt: "młoda dorosła kobieta, ok. 23 lat, ciepły spokojny środkowy rejestr, naturalne tempo, uśmiech w głosie, czysta polszczyzna",
            }),
            seed(Seed {
                persona: PersonaId::beta(),
                name: "Beta",
                age: 22,
                timbre: "lekko wyższa, miękka",
                register: Register::MiddleHigh,
                tempo: 0.96,
                energy: 0.55,
                pitch: 1.0,
                emotion: Emotion::Warm,
                range: &[
                    Emotion::Neutral,
                    Emotion::Warm,
                    Emotion::Joy,
                    Emotion::Empathy,
                    Emotion::Calm,
                    Emotion::Serious,
                ],
                prompt: "młoda dorosła kobieta, ok. 22 lat, pogodna, lekko wyższa i miękka barwa, bardzo wyraźna dykcja, umiarkowane tempo, życzliwa",
            }),
            seed(Seed {
                persona: PersonaId::gama(),
                name: "Gama",
                age: 25,
                timbre: "niższa, miękka",
                register: Register::Low,
                tempo: 0.9,
                energy: 0.4,
                pitch: -1.5,
                emotion: Emotion::Neutral,
                range: &[
                    Emotion::Neutral,
                    Emotion::Calm,
                    Emotion::Curiosity,
                    Emotion::Serious,
                ],
                prompt: "młoda dorosła kobieta, ok. 25 lat, niższy miękki rejestr, wolniejsze przemyślane tempo, rzeczowa",
            }),
            seed(Seed {
                persona: PersonaId::delta(),
                name: "Delta",
                age: 20,
                timbre: "jaśniejsza",
                register: Register::MiddleHigh,
                tempo: 1.1,
                energy: 0.7,
                pitch: 1.5,
                emotion: Emotion::Enthusiasm,
                range: &[
                    Emotion::Neutral,
                    Emotion::Enthusiasm,
                    Emotion::Joy,
                    Emotion::Serious,
                ],
                prompt: "młoda dorosła kobieta, ok. 20 lat, jaśniejsza barwa, żwawe tempo, energiczna i konkretna",
            }),
        ]
    }
}

struct Seed {
    persona: PersonaId,
    name: &'static str,
    age: u8,
    timbre: &'static str,
    register: Register,
    tempo: f32,
    energy: f32,
    pitch: f32,
    emotion: Emotion,
    range: &'static [Emotion],
    prompt: &'static str,
}

fn seed(s: Seed) -> VoiceBible {
    VoiceBible {
        persona: s.persona,
        display_name: s.name.to_owned(),
        lang: "pl-PL".to_owned(),
        perceived_age: s.age,
        timbre: s.timbre.to_owned(),
        register: s.register,
        tempo: s.tempo,
        energy: s.energy,
        pitch_semitones: s.pitch,
        default_emotion: s.emotion,
        emotion_range: s.range.to_vec(),
        voice_prompt: s.prompt.to_owned(),
        provenance: Provenance::BuiltinV0,
        consent: Consent {
            real_person: false,
            note: "głos v0 wbudowany lub zaprojektowany z opisu; brak prawdziwej osoby".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_bibles_are_valid_and_distinct() {
        let all = VoiceBible::builtin_all();
        assert_eq!(all.len(), 4);
        for b in &all {
            b.validate().unwrap();
            assert!(b.emotion_range.contains(&b.default_emotion));
        }
        // Gama najwolniejsza i najniższa, Delta najszybsza (PERSONAS §2.5).
        let gama = VoiceBible::builtin(&PersonaId::gama()).unwrap();
        let delta = VoiceBible::builtin(&PersonaId::delta()).unwrap();
        assert!(gama.tempo < delta.tempo && gama.pitch_semitones < delta.pitch_semitones);
        assert!(VoiceBible::builtin(&PersonaId::new("zeta")).is_none());
    }

    #[test]
    fn validation_rejects_invariant_violations() {
        let mut b = VoiceBible::builtin(&PersonaId::alfa()).unwrap();
        b.perceived_age = 17;
        assert!(b.validate().is_err());
        let mut b = VoiceBible::builtin(&PersonaId::alfa()).unwrap();
        b.voice_prompt.push_str(", cute");
        assert!(b.validate().is_err());
        let mut b = VoiceBible::builtin(&PersonaId::alfa()).unwrap();
        b.consent.real_person = true;
        assert!(b.validate().is_err());
        let mut b = VoiceBible::builtin(&PersonaId::alfa()).unwrap();
        b.energy = 1.5;
        assert!(b.validate().is_err());
    }
}
