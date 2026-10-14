//! Strojenie mowy agentek z Ustawień → Głos: tempo, styl mówienia, pauzy między zdaniami
//! i odrębność głosów. Silniki czytają je przy każdym zdaniu, więc zmiana działa od razu —
//! bez ponownego uruchamiania programu ani rozmowy.

use std::sync::Arc;

use core_config_contract::{ConfigKey, ConfigStore, Scope};
use futures_util::StreamExt;
use voice_tts_contract::{
    DEFAULT_TIMBRES, SharedTuning, TtsTuning, VoiceStyle, character_from_setting, timbre_index,
};

/// Tempo mowy w procentach (60–160).
pub const SPEED_KEY: &str = "voice.tts_speed";
/// Styl mówienia (`calm` / `natural` / `lively`).
pub const STYLE_KEY: &str = "voice.tts_style";
/// Pauza między zdaniami w ms (0–1000).
pub const PAUSE_KEY: &str = "voice.tts_pause";
/// Odrębność głosów agentek (`same` / `subtle` / `distinct`).
pub const CHARACTER_KEY: &str = "voice.tts_character";

/// Głośność mowy w procentach (20–150).
pub const VOLUME_KEY: &str = "voice.tts_volume";

/// Barwy głosu agentek (Supertonic 3): `F1`…`F5`, `M1`…`M5`.
pub const VOICE_KEYS: [&str; 4] = [
    "voice.tts_voice_alfa",
    "voice.tts_voice_beta",
    "voice.tts_voice_gama",
    "voice.tts_voice_delta",
];

const KEYS: [&str; 9] = [
    SPEED_KEY,
    STYLE_KEY,
    PAUSE_KEY,
    CHARACTER_KEY,
    VOLUME_KEY,
    VOICE_KEYS[0],
    VOICE_KEYS[1],
    VOICE_KEYS[2],
    VOICE_KEYS[3],
];

/// Barwy agentek z wartości ustawień (brak albo nieznana nazwa → barwa domyślna agentki).
pub fn timbres_from(values: [Option<&str>; 4]) -> [u8; 4] {
    let mut timbres = DEFAULT_TIMBRES;
    for (slot, value) in timbres.iter_mut().zip(values) {
        if let Some(index) = value.and_then(timbre_index) {
            *slot = index;
        }
    }
    timbres
}

/// Zalecane strojenie programu — używane, dopóki użytkownik niczego nie zmieni.
pub fn recommended() -> TtsTuning {
    let (expressiveness, rhythm) = VoiceStyle::Natural.params();
    TtsTuning {
        speed: 1.0,
        expressiveness,
        rhythm,
        pause_ms: 200,
        character: character_from_setting("subtle"),
        volume: 1.0,
        timbres: DEFAULT_TIMBRES,
    }
}

/// Strojenie z wartości ustawień (brak albo zły typ wartości → zalecane).
pub fn from_values(
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
        timbres: base.timbres,
    }
    .clamped()
}

/// Strojenie śledzące konfigurację.
pub fn tts_tuning(config: &Arc<dyn ConfigStore>) -> SharedTuning {
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
            let mut tuning = from_values(number(0), text(1), number(2), text(3), number(4));
            tuning.timbres = timbres_from([text(5), text(6), text(7), text(8)]);
            handle.set(tuning);
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
    fn agent_timbres_follow_settings_and_fall_back_to_defaults() {
        assert_eq!(timbres_from([None; 4]), DEFAULT_TIMBRES);
        let chosen = timbres_from([Some("M2"), Some("f3"), Some("nie ma"), None]);
        assert_eq!(chosen, [6, 2, DEFAULT_TIMBRES[2], DEFAULT_TIMBRES[3]]);
        let tuning = TtsTuning {
            timbres: chosen,
            ..recommended()
        };
        assert_eq!(tuning.timbre_for("alfa"), Some("M2"));
        assert_eq!(tuning.timbre_for("beta"), Some("F3"));
        assert_eq!(tuning.timbre_for("gama"), Some("F4"));
        assert_eq!(tuning.timbre_for("zofia"), None, "agentka z Kreatora");
    }

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
