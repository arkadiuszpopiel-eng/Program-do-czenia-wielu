//! Strojenie mowy agentek z Ustawień → Głos: tempo, styl mówienia, pauzy między zdaniami
//! i odrębność głosów. Silniki czytają je przy każdym zdaniu, więc zmiana działa od razu —
//! bez ponownego uruchamiania programu ani rozmowy.

use std::sync::Arc;

use core_config_contract::{ConfigKey, ConfigStore, Scope};
use futures_util::StreamExt;
use voice_tts_contract::{SharedTuning, TtsTuning, VoiceStyle, character_from_setting};

/// Tempo mowy w procentach (60–160).
pub(crate) const SPEED_KEY: &str = "voice.tts_speed";
/// Styl mówienia (`calm` / `natural` / `lively`).
pub(crate) const STYLE_KEY: &str = "voice.tts_style";
/// Pauza między zdaniami w ms (0–1000).
pub(crate) const PAUSE_KEY: &str = "voice.tts_pause";
/// Odrębność głosów agentek (`same` / `subtle` / `distinct`).
pub(crate) const CHARACTER_KEY: &str = "voice.tts_character";

/// Głośność mowy w procentach (20–150).
pub(crate) const VOLUME_KEY: &str = "voice.tts_volume";

const KEYS: [&str; 5] = [SPEED_KEY, STYLE_KEY, PAUSE_KEY, CHARACTER_KEY, VOLUME_KEY];

/// Zalecane strojenie programu — używane, dopóki użytkownik niczego nie zmieni.
pub(crate) fn recommended() -> TtsTuning {
    let (expressiveness, rhythm) = VoiceStyle::Natural.params();
    TtsTuning {
        speed: 1.0,
        expressiveness,
        rhythm,
        pause_ms: 200,
        character: character_from_setting("subtle"),
        volume: 1.0,
    }
}

/// Strojenie z wartości ustawień (brak albo zły typ wartości → zalecane).
pub(crate) fn from_values(
    speed: Option<f64>,
    style: Option<&str>,
    pause: Option<f64>,
    character: Option<&str>,
    volume: Option<f64>,
) -> TtsTuning {
    let base = recommended();
    let (expressiveness, rhythm) = style.map_or((base.expressiveness, base.rhythm), |s| {
        VoiceStyle::parse(s).params()
    });
    TtsTuning {
        speed: speed.map_or(base.speed, |percent| (percent / 100.0) as f32),
        expressiveness,
        rhythm,
        pause_ms: pause.map_or(base.pause_ms, |ms| ms.clamp(0.0, 60_000.0) as u32),
        character: character.map_or(base.character, character_from_setting),
        volume: volume.map_or(base.volume, |percent| (percent / 100.0) as f32),
    }
    .clamped()
}

/// Strojenie śledzące konfigurację.
pub(crate) fn tts_tuning(config: &Arc<core_config_impl::FileConfigStore>) -> SharedTuning {
    let shared = SharedTuning::default();
    shared.set(recommended());
    let (config, handle) = (config.clone(), shared.clone());
    tokio::spawn(async move {
        // Jedna obserwacja na wspólny przedrostek kluczy strojenia.
        let mut changes = config.watch("voice.tts_");
        loop {
            let mut values = Vec::with_capacity(KEYS.len());
            for name in KEYS {
                let value = match ConfigKey::new(name) {
                    Ok(key) => config.get(&key, &Scope::Global).await.ok().flatten(),
                    Err(_) => None,
                };
                values.push(value);
            }
            let number = |i: usize| values[i].as_ref().and_then(serde_json::Value::as_f64);
            let text = |i: usize| values[i].as_ref().and_then(serde_json::Value::as_str);
            handle.set(from_values(
                number(0),
                text(1),
                number(2),
                text(3),
                number(4),
            ));
            if changes.next().await.is_none() {
                break;
            }
        }
    });
    shared
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_give_the_recommended_natural_voice() {
        let tuning = from_values(None, None, None, None, None);
        assert_eq!(tuning, recommended().clamped());
        // Zalecane brzmi inaczej niż surowe ustawienia silnika: żywsza intonacja, pauzy, łagodniejsze
        // różnice między agentkami.
        let raw = TtsTuning::default();
        assert!(tuning.expressiveness > raw.expressiveness);
        assert!(tuning.pause_ms > 0);
        assert!(tuning.character < raw.character);
    }

    #[test]
    fn settings_values_map_to_tuning_and_stay_in_range() {
        let tuning = from_values(
            Some(120.0),
            Some("calm"),
            Some(400.0),
            Some("same"),
            Some(60.0),
        );
        assert!((tuning.volume - 0.6).abs() < 1e-6);
        assert!((tuning.speed - 1.2).abs() < 1e-6);
        assert_eq!(
            (tuning.expressiveness, tuning.rhythm),
            VoiceStyle::Calm.params()
        );
        assert_eq!(tuning.pause_ms, 400);
        assert_eq!(tuning.character, 0.0);
        let wild = from_values(
            Some(900.0),
            Some("???"),
            Some(-5.0),
            Some("distinct"),
            Some(900.0),
        );
        assert_eq!(wild.volume, voice_tts_contract::VOLUME_RANGE.1);
        assert_eq!(wild.speed, voice_tts_contract::SPEED_RANGE.1);
        assert_eq!(wild.pause_ms, 0);
        assert_eq!(wild.character, 1.0);
        assert_eq!(wild.expressiveness, VoiceStyle::Natural.params().0);
    }
}
