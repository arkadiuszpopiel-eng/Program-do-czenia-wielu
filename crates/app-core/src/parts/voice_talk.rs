//! Ustawienia rozmowy z Ustawień → Głos (`voice.talk_*`): Smart Turn, próg pewności końca
//! wypowiedzi, najdłuższa cisza, wybór silnika rozpoznawania i język rozpoznawania. Potok głosu czyta je przy starcie
//! rozmowy albo dyktowania — zmiana działa od następnego włączenia mikrofonu, bez restartu programu.

use std::sync::Arc;

use app_voice::TalkSettings;
use core_config_contract::{ConfigKey, ConfigStore, Scope};
use futures_util::StreamExt;

/// Czy koniec wypowiedzi ocenia Smart Turn.
pub(crate) const SMART_TURN_KEY: &str = "voice.talk_smart_turn";
/// Próg pewności końca wypowiedzi w procentach (30–90).
pub(crate) const THRESHOLD_KEY: &str = "voice.talk_turn_threshold";
/// Najdłuższa cisza w ms (500–6000).
pub(crate) const MAX_SILENCE_KEY: &str = "voice.talk_max_silence";
/// Silnik rozpoznawania (`auto` / `whisper`).
pub(crate) const STT_ENGINE_KEY: &str = "voice.talk_stt_engine";
/// Język rozpoznawania (`pl` / `en` / `auto`).
pub(crate) const LANGUAGE_KEY: &str = "voice.talk_language";

const KEYS: [&str; 5] = [
    SMART_TURN_KEY,
    THRESHOLD_KEY,
    MAX_SILENCE_KEY,
    STT_ENGINE_KEY,
    LANGUAGE_KEY,
];

/// Ustawienia rozmowy śledzące konfigurację.
pub(crate) fn talk_settings(config: &Arc<core_config_impl::FileConfigStore>) -> TalkSettings {
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

#[cfg(test)]
mod tests {
    #[test]
    fn keys_exist_in_the_settings_tree() {
        let pages = include_str!("../../data/settings-pages.json");
        for key in super::KEYS {
            assert!(pages.contains(&format!("\"{key}\"")), "{key}");
        }
    }
}
