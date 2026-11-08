//! Styl mówienia: znaczniki (emocja, tempo, energia) → parametry silnika (PLAN §6.7).
//!
//! Znaczniki w tekście mają postać `[emocja:radość]`, `[tempo:wolno]`, `[energia:wysoka]`
//! (także nazwy angielskie). Mapowanie na silnik opisuje tabela [`EngineStyleTable`]; parametr,
//! którego silnik nie ma, jest pomijany (degradacja), nigdy czytany jako tekst.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Emocja wypowiedzi.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Emotion {
    /// Neutralnie.
    Neutral,
    /// Ciepło, uśmiech w głosie.
    Warm,
    /// Radość.
    Joy,
    /// Spokój.
    Calm,
    /// Powaga (np. komunikaty o ryzyku — bez pośpiechu).
    Serious,
    /// Troska, empatia.
    Empathy,
    /// Entuzjazm.
    Enthusiasm,
    /// Ciekawość.
    Curiosity,
    /// Smutek.
    Sadness,
}

impl Emotion {
    /// Wszystkie emocje.
    pub const ALL: [Emotion; 9] = [
        Emotion::Neutral,
        Emotion::Warm,
        Emotion::Joy,
        Emotion::Calm,
        Emotion::Serious,
        Emotion::Empathy,
        Emotion::Enthusiasm,
        Emotion::Curiosity,
        Emotion::Sadness,
    ];

    /// Parsuje wartość znacznika (PL lub EN, bez rozróżniania wielkości liter).
    pub fn from_tag(value: &str) -> Option<Self> {
        let v = value.trim().to_lowercase();
        let e = match v.as_str() {
            "neutralnie" | "neutralna" | "neutral" => Self::Neutral,
            "ciepło" | "ciepła" | "warm" => Self::Warm,
            "radość" | "radośnie" | "joy" | "happy" => Self::Joy,
            "spokój" | "spokojnie" | "calm" => Self::Calm,
            "powaga" | "poważnie" | "serious" => Self::Serious,
            "troska" | "empatia" | "empathy" => Self::Empathy,
            "entuzjazm" | "entuzjastycznie" | "enthusiasm" | "excited" => Self::Enthusiasm,
            "ciekawość" | "curiosity" | "curious" => Self::Curiosity,
            "smutek" | "smutno" | "sadness" | "sad" => Self::Sadness,
            _ => return None,
        };
        Some(e)
    }
}

/// Tempo względne (znacznik).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tempo {
    /// Wolniej.
    Slow,
    /// Naturalnie.
    Normal,
    /// Szybciej.
    Fast,
}

impl Tempo {
    /// Parsuje wartość znacznika (PL/EN).
    pub fn from_tag(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "wolno" | "wolniej" | "slow" => Some(Self::Slow),
            "normalnie" | "naturalnie" | "normal" => Some(Self::Normal),
            "szybko" | "szybciej" | "żwawo" | "fast" => Some(Self::Fast),
            _ => None,
        }
    }
}

/// Energia względna (znacznik).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Energy {
    /// Niska (ciszej, spokojniej).
    Low,
    /// Normalna.
    Normal,
    /// Wysoka.
    High,
}

impl Energy {
    /// Parsuje wartość znacznika (PL/EN).
    pub fn from_tag(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "niska" | "nisko" | "low" => Some(Self::Low),
            "normalna" | "normalnie" | "normal" => Some(Self::Normal),
            "wysoka" | "wysoko" | "high" => Some(Self::High),
            _ => None,
        }
    }
}

/// Znaczniki stylu obowiązujące dla zdania (brak = wartość z biblii głosu).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StyleTags {
    /// Emocja.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emotion: Option<Emotion>,
    /// Tempo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo: Option<Tempo>,
    /// Energia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy: Option<Energy>,
}

/// Silnik TTS, dla którego mapujemy styl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EngineKind {
    /// Pocket TTS + model PL (lokalnie, CPU).
    PocketTts,
    /// Piper pl_PL (zapas lokalny).
    Piper,
    /// ElevenLabs (chmura; znaczniki audio w tekście).
    ElevenLabs,
    /// Azure pl-PL (SSML: prosody + express-as).
    Azure,
    /// Cartesia (chmura; kontrola szybkości i emocji).
    Cartesia,
    /// Silnik bez żadnych kontroli stylu.
    Generic,
}

/// Dozwolony zakres parametru silnika.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParamRange {
    /// Minimum.
    pub min: f32,
    /// Maksimum.
    pub max: f32,
}

impl ParamRange {
    /// Przycina wartość do zakresu (NaN → minimum).
    pub fn clamp(&self, value: f32) -> f32 {
        if value.is_nan() {
            return self.min;
        }
        value.max(self.min).min(self.max)
    }
}

/// Znacznik emocji w języku silnika (np. `cheerful` dla Azure, `[warmly]` dla ElevenLabs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EmotionTag {
    /// Emocja.
    pub emotion: Emotion,
    /// Znacznik silnika.
    pub tag: String,
}

/// Tabela możliwości stylu silnika (per silnik; `None` = parametr nieobsługiwany).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EngineStyleTable {
    /// Silnik.
    pub engine: EngineKind,
    /// Mnożnik tempa.
    pub rate: Option<ParamRange>,
    /// Wysokość w półtonach.
    pub pitch_semitones: Option<ParamRange>,
    /// Wzmocnienie w dB.
    pub gain_db: Option<ParamRange>,
    /// Intensywność stylu 0–1 (np. `style` w ElevenLabs).
    pub intensity: Option<ParamRange>,
    /// Obsługiwane emocje.
    pub emotions: Vec<EmotionTag>,
}

impl EngineStyleTable {
    /// Tabela silnika bez żadnych kontroli (wszystko pomijane).
    pub fn neutral(engine: EngineKind) -> Self {
        Self {
            engine,
            rate: None,
            pitch_semitones: None,
            gain_db: None,
            intensity: None,
            emotions: Vec::new(),
        }
    }

    /// Znacznik silnika dla emocji, jeśli obsługiwana.
    pub fn emotion_tag(&self, emotion: Emotion) -> Option<&str> {
        self.emotions
            .iter()
            .find(|t| t.emotion == emotion)
            .map(|t| t.tag.as_str())
    }
}

/// Parametry silnika dla jednego zdania.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpeechStyle {
    /// Silnik.
    pub engine: EngineKind,
    /// Mnożnik tempa (1.0 = naturalne).
    pub rate: f32,
    /// Wysokość w półtonach.
    pub pitch_semitones: f32,
    /// Wzmocnienie w dB.
    pub gain_db: f32,
    /// Intensywność stylu (jeśli silnik ją ma).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intensity: Option<f32>,
    /// Znacznik emocji silnika (jeśli silnik ją ma).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emotion_tag: Option<String>,
}

impl SpeechStyle {
    /// Styl neutralny (bez modyfikacji).
    pub fn neutral(engine: EngineKind) -> Self {
        Self {
            engine,
            rate: 1.0,
            pitch_semitones: 0.0,
            gain_db: 0.0,
            intensity: None,
            emotion_tag: None,
        }
    }
}

/// Wynik planisty stylu: parametry + lista znaczników, których silnik nie obsłużył.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StylePlan {
    /// Parametry silnika.
    pub style: SpeechStyle,
    /// Nieobsłużone znaczniki (np. `emocja:radość`) — do zdarzenia `style_unsupported`.
    pub unsupported: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags_pl_and_en() {
        assert_eq!(Emotion::from_tag("Radość"), Some(Emotion::Joy));
        assert_eq!(Emotion::from_tag("sad"), Some(Emotion::Sadness));
        assert_eq!(Emotion::from_tag("wściekłość"), None);
        assert_eq!(Tempo::from_tag("wolno"), Some(Tempo::Slow));
        assert_eq!(Tempo::from_tag("turbo"), None);
        assert_eq!(Energy::from_tag("HIGH"), Some(Energy::High));
        for e in Emotion::ALL {
            let json = serde_json::to_string(&e).unwrap();
            assert_eq!(serde_json::from_str::<Emotion>(&json).unwrap(), e);
        }
    }

    #[test]
    fn range_clamps_and_table_lookup() {
        let r = ParamRange { min: 0.5, max: 2.0 };
        assert_eq!(r.clamp(3.0), 2.0);
        assert_eq!(r.clamp(0.1), 0.5);
        assert_eq!(r.clamp(f32::NAN), 0.5);
        let mut t = EngineStyleTable::neutral(EngineKind::Azure);
        assert_eq!(t.emotion_tag(Emotion::Joy), None);
        t.emotions.push(EmotionTag {
            emotion: Emotion::Joy,
            tag: "cheerful".into(),
        });
        assert_eq!(t.emotion_tag(Emotion::Joy), Some("cheerful"));
        assert_eq!(SpeechStyle::neutral(EngineKind::Generic).rate, 1.0);
    }
}
