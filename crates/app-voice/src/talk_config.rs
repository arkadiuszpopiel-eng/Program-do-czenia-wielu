//! Ustawienia rozmowy z Ustawień → Głos (`voice.talk_*`): Smart Turn, próg pewności końca
//! wypowiedzi, najdłuższa cisza, wybór silnika rozpoznawania i język rozpoznawania. Potok głosu czyta je przy starcie
//! rozmowy albo dyktowania — zmiana działa od następnego włączenia mikrofonu, bez restartu programu.

use std::sync::Arc;

use core_config_contract::{ConfigKey, ConfigStore, Scope};
use futures_util::StreamExt;

use crate::TalkSettings;

/// Czy koniec wypowiedzi ocenia Smart Turn.
pub const SMART_TURN_KEY: &str = "voice.talk_smart_turn";
/// Próg pewności końca wypowiedzi w procentach (30–90).
pub const THRESHOLD_KEY: &str = "voice.talk_turn_threshold";
/// Najdłuższa cisza w ms (500–6000).
pub const MAX_SILENCE_KEY: &str = "voice.talk_max_silence";
/// Silnik rozpoznawania (`auto` / `whisper`).
pub const STT_ENGINE_KEY: &str = "voice.talk_stt_engine";
/// Język rozpoznawania (`pl` / `en` / `auto`).
pub const LANGUAGE_KEY: &str = "voice.talk_language";

const KEYS: [&str; 5] = [
    SMART_TURN_KEY,
    THRESHOLD_KEY,
    MAX_SILENCE_KEY,
    STT_ENGINE_KEY,
    LANGUAGE_KEY,
];

/// Ustawienia rozmowy śledzące konfigurację.
pub fn talk_settings(config: &Arc<dyn ConfigStore>) -> TalkSettings {
    let shared = TalkSettings::default();
    let (config, handle) = (config.clone(), shared.clone());
    tokio::spawn(async move {
        let mut changes = config.watch("voice.talk_");
        loop {
            let mut values = Vec::with_capacity(KEYS.len());
            for name in KEYS {
                let value = match ConfigKey::new(name) {
                    Ok(key) => config.get(&key, &Scope::Global).await.ok().flatten(),
                    Err(_) => None,
                };
                values.push(value);
            }
            handle.apply_values(&values);
            if changes.next().await.is_none() {
                break;
            }
        }
    });
    shared
}

/// Klucz ustawienia redukcji szumów mikrofonu (`off` / `standard` / `strong`).
pub const NOISE_KEY: &str = "voice.noise_reduction";

/// Ustawienie redukcji szumów śledzące konfigurację: test mikrofonu i rozmowa czytają je przy
/// każdym starcie, więc zmiana w Ustawieniach działa bez restartu programu.
pub fn noise_setting<C: ConfigStore + 'static>(config: Arc<C>) -> voice_dsp_contract::NoiseSetting {
    let setting = voice_dsp_contract::NoiseSetting::default();
    let shared = setting.clone();
    tokio::spawn(async move {
        let Ok(key) = ConfigKey::new(NOISE_KEY) else {
            return;
        };
        let mut changes = config.watch(NOISE_KEY);
        loop {
            let value = config.get(&key, &Scope::Global).await.ok().flatten();
            if let Some(text) = value.as_ref().and_then(|v| v.as_str()) {
                shared.set(voice_dsp_contract::NoiseReduction::parse(text));
            }
            if changes.next().await.is_none() {
                break;
            }
        }
    });
    setting
}
