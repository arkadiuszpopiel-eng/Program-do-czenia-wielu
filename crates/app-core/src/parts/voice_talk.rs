//! Ustawienia rozmowy z Ustawień → Głos (`voice.talk_*`): Smart Turn, próg pewności końca
//! wypowiedzi, najdłuższa cisza i wybór silnika rozpoznawania. Potok głosu czyta je przy starcie
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

const KEYS: [&str; 4] = [
    SMART_TURN_KEY,
    THRESHOLD_KEY,
    MAX_SILENCE_KEY,
    STT_ENGINE_KEY,
];

/// Wpisuje wartości ustawień do uchwytu (brak albo zły typ wartości → bez zmiany).
pub(crate) fn apply(talk: &TalkSettings, values: &[Option<serde_json::Value>]) {
    let at = |i: usize| values.get(i).and_then(Option::as_ref);
    if let Some(on) = at(0).and_then(serde_json::Value::as_bool) {
        talk.set_smart_turn(on);
    }
    if let Some(pct) = at(1).and_then(serde_json::Value::as_f64) {
        talk.set_threshold_pct(pct);
    }
    if let Some(ms) = at(2).and_then(serde_json::Value::as_f64) {
        talk.set_max_silence_ms(ms);
    }
    if let Some(engine) = at(3).and_then(serde_json::Value::as_str) {
        talk.set_stt_engine(engine);
    }
}

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
            apply(&handle, &values);
            if changes.next().await.is_none() {
                break;
            }
        }
    });
    shared
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn values_reach_the_handle_and_bad_types_are_ignored() {
        let talk = TalkSettings::default();
        apply(
            &talk,
            &[
                Some(json!(false)),
                Some(json!(65)),
                Some(json!(3200)),
                Some(json!("whisper")),
            ],
        );
        assert!(!talk.smart_turn() && talk.whisper_only());
        assert_eq!((talk.threshold_pct(), talk.max_silence_ms()), (65, 3_200));
        apply(
            &talk,
            &[
                Some(json!("tak")),
                None,
                Some(json!("dużo")),
                Some(json!(7)),
            ],
        );
        assert!(!talk.smart_turn() && talk.whisper_only());
        assert_eq!((talk.threshold_pct(), talk.max_silence_ms()), (65, 3_200));
        apply(&talk, &[Some(json!(true)), None, None, Some(json!("auto"))]);
        assert!(talk.smart_turn() && !talk.whisper_only());
    }

    #[test]
    fn keys_exist_in_the_settings_tree() {
        let pages = include_str!("../../data/settings-pages.json");
        for key in KEYS {
            assert!(pages.contains(&format!("\"{key}\"")), "{key}");
        }
    }
}
