//! Ustawienia rozmowy z Ustawień → Głos: czy koniec wypowiedzi ocenia Smart Turn, od jakiej
//! pewności, ile najdłużej czekać w ciszy i który silnik rozpoznaje mowę. Potok czyta je przy
//! starcie rozmowy albo dyktowania, więc zmiana działa od następnego włączenia mikrofonu.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

use voice_stt_contract::LangMode;
use voice_turn_contract::{Patience, PatienceCfg, TurnCfg};

/// Zakres progu pewności końca wypowiedzi (%).
pub const THRESHOLD_RANGE: (u8, u8) = (30, 90);
/// Zakres najdłuższej ciszy (ms).
pub const MAX_SILENCE_RANGE: (u32, u32) = (500, 6_000);
/// Cisza przy pewnym końcu i przy umiarkowanej pewności (wartości z Delty AI).
const MIN_SILENCE_MS: u64 = 250;
const BASE_SILENCE_MS: u64 = 350;
const HESITATION_BONUS_MS: u64 = 400;

#[derive(Debug)]
struct Inner {
    smart_turn: AtomicBool,
    threshold_pct: AtomicU8,
    max_silence_ms: AtomicU32,
    whisper_only: AtomicBool,
    /// Język rozpoznawania: 0 — automatycznie, 1 — polski, 2 — angielski.
    language: AtomicU8,
}

/// Ustawienia rozmowy współdzielone między Ustawieniami a potokiem głosu.
#[derive(Debug, Clone)]
pub struct TalkSettings(Arc<Inner>);

impl Default for TalkSettings {
    fn default() -> Self {
        Self(Arc::new(Inner {
            smart_turn: AtomicBool::new(true),
            threshold_pct: AtomicU8::new(50),
            max_silence_ms: AtomicU32::new(2_000),
            whisper_only: AtomicBool::new(false),
            language: AtomicU8::new(1),
        }))
    }
}

impl TalkSettings {
    /// Czy koniec wypowiedzi ocenia Smart Turn (gdy są jego pliki).
    pub fn smart_turn(&self) -> bool {
        self.0.smart_turn.load(Ordering::Relaxed)
    }

    /// Włącza albo wyłącza Smart Turn.
    pub fn set_smart_turn(&self, on: bool) {
        self.0.smart_turn.store(on, Ordering::Relaxed);
    }

    /// Próg pewności końca wypowiedzi (%).
    pub fn threshold_pct(&self) -> u8 {
        self.0.threshold_pct.load(Ordering::Relaxed)
    }

    /// Ustawia próg (przycięty do zakresu).
    pub fn set_threshold_pct(&self, pct: f64) {
        let (lo, hi) = THRESHOLD_RANGE;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let value = pct.clamp(f64::from(lo), f64::from(hi)).round() as u8;
        self.0.threshold_pct.store(value, Ordering::Relaxed);
    }

    /// Najdłuższa cisza, po której wypowiedź kończy się zawsze (ms).
    pub fn max_silence_ms(&self) -> u32 {
        self.0.max_silence_ms.load(Ordering::Relaxed)
    }

    /// Ustawia najdłuższą ciszę (przyciętą do zakresu).
    pub fn set_max_silence_ms(&self, ms: f64) {
        let (lo, hi) = MAX_SILENCE_RANGE;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let value = ms.clamp(f64::from(lo), f64::from(hi)).round() as u32;
        self.0.max_silence_ms.store(value, Ordering::Relaxed);
    }

    /// Czy rozpoznawać samym Whisperem (bez Qwen3-ASR).
    pub fn whisper_only(&self) -> bool {
        self.0.whisper_only.load(Ordering::Relaxed)
    }

    /// Wybór silnika rozpoznawania: `whisper` — sam Whisper; cokolwiek innego — najlepszy dostępny.
    pub fn set_stt_engine(&self, value: &str) {
        self.0
            .whisper_only
            .store(value.eq_ignore_ascii_case("whisper"), Ordering::Relaxed);
    }

    /// Język rozpoznawania mowy (domyślnie polski: silnik nie zgaduje języka krótkich zdań,
    /// a strażnik wie, jakiego języka się spodziewać).
    pub fn language(&self) -> LangMode {
        match self.0.language.load(Ordering::Relaxed) {
            0 => LangMode::Auto,
            2 => LangMode::En,
            _ => LangMode::Pl,
        }
    }

    /// Wybór języka: `auto`, `en`, cokolwiek innego — polski.
    pub fn set_language(&self, value: &str) {
        let code = match value.trim().to_ascii_lowercase().as_str() {
            "auto" => 0,
            "en" => 2,
            _ => 1,
        };
        self.0.language.store(code, Ordering::Relaxed);
    }

    /// Ustawienia detektora końca wypowiedzi dla modelu słuchającego dźwięku.
    pub fn turn_cfg(&self) -> TurnCfg {
        let max_ms = u64::from(self.max_silence_ms());
        let base_ms = BASE_SILENCE_MS.min(max_ms);
        let eot = f32::from(self.threshold_pct()) / 100.0;
        TurnCfg {
            patience: Patience::Custom(PatienceCfg {
                min_silence_ms: MIN_SILENCE_MS.min(base_ms),
                base_ms,
                hesitation_bonus_ms: HESITATION_BONUS_MS,
                low_prob_bonus_ms: max_ms - base_ms,
                max_ms,
            }),
            eot_threshold: eot,
            // „Pewny koniec” zawsze wyraźnie powyżej progu, ale nie nieosiągalny.
            confident_threshold: (eot + 0.05).clamp(0.85, 0.97),
            ..TurnCfg::default()
        }
    }

    /// Wpisuje wartości ustawień `voice.talk_*` w kolejności: Smart Turn, próg, cisza, silnik,
    /// język (brak albo zły typ wartości → bez zmiany).
    pub fn apply_values(&self, values: &[Option<serde_json::Value>]) {
        let at = |i: usize| values.get(i).and_then(Option::as_ref);
        if let Some(on) = at(0).and_then(serde_json::Value::as_bool) {
            self.set_smart_turn(on);
        }
        if let Some(pct) = at(1).and_then(serde_json::Value::as_f64) {
            self.set_threshold_pct(pct);
        }
        if let Some(ms) = at(2).and_then(serde_json::Value::as_f64) {
            self.set_max_silence_ms(ms);
        }
        if let Some(engine) = at(3).and_then(serde_json::Value::as_str) {
            self.set_stt_engine(engine);
        }
        if let Some(language) = at(4).and_then(serde_json::Value::as_str) {
            self.set_language(language);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_tuned_conversation_preset() {
        let talk = TalkSettings::default();
        assert!(talk.smart_turn() && !talk.whisper_only());
        let cfg = talk.turn_cfg();
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.patience.cfg(), Patience::Conversation.cfg());
        assert!((cfg.eot_threshold - 0.5).abs() < 1e-6);
        assert!((cfg.confident_threshold - 0.85).abs() < 1e-6);
    }

    #[test]
    fn values_are_clamped_and_always_valid() {
        let talk = TalkSettings::default();
        for (pct, ms) in [
            (0.0, 0.0),
            (30.0, 500.0),
            (90.0, 6_000.0),
            (250.0, 99_999.0),
        ] {
            talk.set_threshold_pct(pct);
            talk.set_max_silence_ms(ms);
            let cfg = talk.turn_cfg();
            assert!(cfg.validate().is_ok(), "{pct} {ms}: {cfg:?}");
            let p = cfg.patience.cfg();
            assert_eq!(p.base_ms + p.low_prob_bonus_ms, p.max_ms);
            assert!(cfg.confident_threshold > cfg.eot_threshold);
        }
        assert_eq!((talk.threshold_pct(), talk.max_silence_ms()), (90, 6_000));
        talk.set_threshold_pct(1.0);
        talk.set_max_silence_ms(1.0);
        assert_eq!((talk.threshold_pct(), talk.max_silence_ms()), (30, 500));
    }

    #[test]
    fn engine_choice_and_shared_handle() {
        let talk = TalkSettings::default();
        let other = talk.clone();
        other.set_stt_engine("Whisper");
        other.set_smart_turn(false);
        assert!(talk.whisper_only() && !talk.smart_turn());
        other.set_stt_engine("auto");
        assert!(!talk.whisper_only());
        assert_eq!(talk.language(), LangMode::Pl, "domyślnie polski");
        other.set_language("EN");
        assert_eq!(talk.language(), LangMode::En);
        other.set_language("auto");
        assert_eq!(talk.language(), LangMode::Auto);
        other.set_language("cokolwiek");
        assert_eq!(talk.language(), LangMode::Pl);
    }
}

#[cfg(test)]
mod apply_tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn values_reach_the_handle_and_bad_types_are_ignored() {
        let talk = TalkSettings::default();
        talk.apply_values(&[
            Some(json!(false)),
            Some(json!(65)),
            Some(json!(3200)),
            Some(json!("whisper")),
        ]);
        assert!(!talk.smart_turn() && talk.whisper_only());
        assert_eq!((talk.threshold_pct(), talk.max_silence_ms()), (65, 3_200));
        talk.apply_values(&[
            Some(json!("tak")),
            None,
            Some(json!("dużo")),
            Some(json!(7)),
        ]);
        assert!(!talk.smart_turn() && talk.whisper_only());
        assert_eq!((talk.threshold_pct(), talk.max_silence_ms()), (65, 3_200));
        talk.apply_values(&[Some(json!(true)), None, None, Some(json!("auto"))]);
        assert!(talk.smart_turn() && !talk.whisper_only());
        assert_eq!(talk.language().code(), "pl");
        talk.apply_values(&[None, None, None, None, Some(json!("en"))]);
        assert_eq!(talk.language().code(), "en");
        talk.apply_values(&[None, None, None, None, Some(json!(12))]);
        assert_eq!(talk.language().code(), "en", "zły typ — bez zmiany");
    }
}
