//! Strojenie mowy przez użytkownika (Ustawienia → Głos): tempo, ekspresja, rytm, pauzy i to, jak
//! bardzo głosy agentek różnią się od siebie. Wartości domyślne odpowiadają zachowaniu sprzed
//! wprowadzenia strojenia; zalecane ustawienia programu wybiera aplikacja ([`VoiceStyle`]).

use std::sync::{Arc, PoisonError, RwLock};

/// Zakres tempa mowy (1.0 = tempo głosu bazowego).
pub const SPEED_RANGE: (f32, f32) = (0.6, 1.6);
/// Zakres głośności mowy (mnożnik amplitudy).
pub const VOLUME_RANGE: (f32, f32) = (0.2, 1.5);
/// Najdłuższa pauza między zdaniami (ms).
pub const MAX_PAUSE_MS: u32 = 1_000;

/// Parametry syntezy wspólne dla wszystkich agentek.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TtsTuning {
    /// Tempo mowy (mnożnik; > 1 = szybciej).
    pub speed: f32,
    /// Ekspresja — zmienność intonacji (Piper: `noise_scale`, 0.3–1.0).
    pub expressiveness: f32,
    /// Zmienność rytmu — długości głosek (Piper: `noise_w`, 0.3–1.2).
    pub rhythm: f32,
    /// Pauza po każdym zdaniu (ms).
    pub pause_ms: u32,
    /// Odrębność głosów agentek: 0 = wszystkie mówią głosem bazowym (bez przestrajania, najczystszy
    /// dźwięk), 1 = pełne różnice wysokości i tempa z presetów.
    pub character: f32,
    /// Głośność mowy (mnożnik amplitudy; 1.0 = bez zmiany).
    pub volume: f32,
    /// Barwy Supertonic agentek wbudowanych (Alfa, Beta, Gama, Delta) jako numery w
    /// [`crate::SUPERTONIC_ALL`]; wybierane w Ustawieniach → Głos.
    pub timbres: [u8; 4],
}

/// Barwy domyślne: Alfa F1, Beta F2, Gama F4, Delta F5.
pub const DEFAULT_TIMBRES: [u8; 4] = [0, 1, 3, 4];

/// Numer barwy Supertonic o podanej nazwie (`F1`…`M5`).
#[must_use]
pub fn timbre_index(name: &str) -> Option<u8> {
    crate::SUPERTONIC_ALL
        .iter()
        .position(|v| v.eq_ignore_ascii_case(name))
        .and_then(|i| u8::try_from(i).ok())
}

impl Default for TtsTuning {
    fn default() -> Self {
        Self {
            speed: 1.0,
            expressiveness: 0.667,
            rhythm: 0.8,
            pause_ms: 0,
            character: 1.0,
            volume: 1.0,
            timbres: DEFAULT_TIMBRES,
        }
    }
}

impl TtsTuning {
    /// Wartości sprowadzone do dopuszczalnych zakresów (NaN → domyślne).
    #[must_use]
    pub fn clamped(self) -> Self {
        let base = Self::default();
        let fix = |value: f32, fallback: f32, low: f32, high: f32| {
            if value.is_finite() {
                value.clamp(low, high)
            } else {
                fallback
            }
        };
        Self {
            speed: fix(self.speed, base.speed, SPEED_RANGE.0, SPEED_RANGE.1),
            expressiveness: fix(self.expressiveness, base.expressiveness, 0.3, 1.0),
            rhythm: fix(self.rhythm, base.rhythm, 0.3, 1.2),
            pause_ms: self.pause_ms.min(MAX_PAUSE_MS),
            character: fix(self.character, base.character, 0.0, 1.0),
            volume: fix(self.volume, base.volume, VOLUME_RANGE.0, VOLUME_RANGE.1),
            timbres: self.timbres.map(|t| t.min(9)),
        }
    }

    /// Barwa Supertonic wybrana dla agentki wbudowanej; dla innych agentek `None` (zostaje barwa
    /// z łańcucha głosów).
    #[must_use]
    pub fn timbre_for(self, persona: &str) -> Option<&'static str> {
        let slot = ["alfa", "beta", "gama", "delta"]
            .iter()
            .position(|p| *p == persona)?;
        crate::SUPERTONIC_ALL
            .get(usize::from(self.timbres[slot]))
            .copied()
    }

    /// Wysokość albo tempo presetu agentki po uwzględnieniu odrębności głosów: przy `character`
    /// równym 0 każda wartość wraca do 1.0 (głos bazowy).
    #[must_use]
    pub fn blend(self, preset: f32) -> f32 {
        1.0 + (preset - 1.0) * self.character.clamp(0.0, 1.0)
    }

    /// Odcisk strojenia do kluczy pamięci podręcznej: nagranie zapisane przy innych
    /// ustawieniach nie może zostać odtworzone po ich zmianie.
    #[must_use]
    pub fn fingerprint(self) -> String {
        let t = self.clamped();
        format!(
            "{:.3}|{:.3}|{:.3}|{}|{:.3}|{:.3}|{:?}",
            t.speed, t.expressiveness, t.rhythm, t.pause_ms, t.character, t.volume, t.timbres
        )
    }

    /// Nakłada głośność na próbki (z ograniczeniem do zakresu −1…1, bez przesterowania „zawijaniem”).
    pub fn apply_volume(self, pcm: &mut [f32]) {
        let gain = self.volume.clamp(VOLUME_RANGE.0, VOLUME_RANGE.1);
        if (gain - 1.0).abs() < 1e-3 {
            return;
        }
        for sample in pcm {
            *sample = (*sample * gain).clamp(-1.0, 1.0);
        }
    }
}

/// Styl mówienia wybierany w Ustawieniach — gotowe zestawy ekspresji i rytmu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoiceStyle {
    /// Spokojny, równy — mniej zmienności.
    Calm,
    /// Naturalny (zalecany) — żywsza intonacja i rytm niż domyślne ustawienia silnika.
    #[default]
    Natural,
    /// Żywy — najwięcej ekspresji.
    Lively,
}

impl VoiceStyle {
    /// Styl z wartości ustawienia (`calm` / `natural` / `lively`); nieznana → naturalny.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        match text {
            "calm" => Self::Calm,
            "lively" => Self::Lively,
            _ => Self::Natural,
        }
    }

    /// Para (ekspresja, rytm).
    #[must_use]
    pub fn params(self) -> (f32, f32) {
        match self {
            Self::Calm => (0.5, 0.7),
            Self::Natural => (0.75, 0.95),
            Self::Lively => (0.92, 1.05),
        }
    }
}

/// Odrębność głosów agentek z wartości ustawienia (`same` / `subtle` / `distinct`).
#[must_use]
pub fn character_from_setting(text: &str) -> f32 {
    match text {
        "same" => 0.0,
        "distinct" => 1.0,
        _ => 0.45,
    }
}

/// Strojenie współdzielone między Ustawieniami a silnikami — zmiana działa od następnego zdania.
#[derive(Debug, Clone, Default)]
pub struct SharedTuning(Arc<RwLock<TtsTuning>>);

impl SharedTuning {
    /// Bieżące strojenie.
    #[must_use]
    pub fn get(&self) -> TtsTuning {
        *self.0.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// Ustawia strojenie (wartości są sprowadzane do zakresów).
    pub fn set(&self, tuning: TtsTuning) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = tuning.clamped();
    }

    /// Zmienia strojenie w miejscu.
    pub fn update(&self, change: impl FnOnce(&mut TtsTuning)) {
        let mut current = self.get();
        change(&mut current);
        self.set(current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_the_old_sound_and_values_are_clamped() {
        let base = TtsTuning::default();
        assert_eq!(base.blend(1.12), 1.12);
        assert_eq!(base.pause_ms, 0);
        let wild = TtsTuning {
            speed: 9.0,
            expressiveness: f32::NAN,
            rhythm: -1.0,
            pause_ms: 60_000,
            character: 3.0,
            volume: 40.0,
            ..TtsTuning::default()
        }
        .clamped();
        assert_eq!(wild.volume, VOLUME_RANGE.1);
        let mut pcm = [0.5f32, -0.9, 0.0];
        wild.apply_volume(&mut pcm);
        assert_eq!(pcm, [0.75, -1.0, 0.0]);
        // Odcisk zmienia się z każdym parametrem, a równe strojenia mają równy odcisk.
        assert_eq!(base.fingerprint(), TtsTuning::default().fingerprint());
        assert_ne!(base.fingerprint(), wild.fingerprint());
        let quieter = TtsTuning {
            volume: 0.5,
            ..base
        };
        assert_ne!(base.fingerprint(), quieter.fingerprint());
        let mut same = [0.5f32, -0.9];
        base.apply_volume(&mut same);
        assert_eq!(same, [0.5, -0.9]);
        assert_eq!(wild.speed, SPEED_RANGE.1);
        assert_eq!(wild.expressiveness, base.expressiveness);
        assert_eq!(wild.rhythm, 0.3);
        assert_eq!(wild.pause_ms, MAX_PAUSE_MS);
        assert_eq!(wild.character, 1.0);
    }

    #[test]
    fn character_scales_the_difference_between_agents() {
        let same = TtsTuning {
            character: character_from_setting("same"),
            ..TtsTuning::default()
        };
        assert_eq!(same.blend(1.14), 1.0);
        assert_eq!(same.blend(0.9), 1.0);
        let subtle = TtsTuning {
            character: character_from_setting("subtle"),
            ..TtsTuning::default()
        };
        let pitch = subtle.blend(1.14);
        assert!(pitch > 1.05 && pitch < 1.08, "{pitch}");
        assert_eq!(character_from_setting("distinct"), 1.0);
        assert_eq!(
            character_from_setting("???"),
            character_from_setting("subtle")
        );
    }

    #[test]
    fn styles_differ_and_shared_tuning_is_visible_to_every_holder() {
        let (calm, natural, lively) = (
            VoiceStyle::Calm.params(),
            VoiceStyle::parse("natural").params(),
            VoiceStyle::parse("lively").params(),
        );
        assert!(calm.0 < natural.0 && natural.0 < lively.0);
        assert!(calm.1 < natural.1 && natural.1 < lively.1);
        assert_eq!(VoiceStyle::parse("nieznany"), VoiceStyle::Natural);
        let shared = SharedTuning::default();
        let other = shared.clone();
        shared.update(|t| {
            t.speed = 1.2;
            t.pause_ms = 250;
        });
        assert_eq!(other.get().speed, 1.2);
        assert_eq!(other.get().pause_ms, 250);
    }
}
