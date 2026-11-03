//! Ustawienia rozmowy (`voice.talk_*`, `parts/voice_talk.rs`) mają swoje miejsca w drzewie Ustawień.

const KEYS: [&str; 5] = [
    "voice.talk_smart_turn",
    "voice.talk_turn_threshold",
    "voice.talk_max_silence",
    "voice.talk_stt_engine",
    "voice.talk_language",
];

#[test]
fn keys_exist_in_the_settings_tree() {
    let pages = include_str!("../data/settings-pages.json");
    for key in KEYS {
        assert!(pages.contains(&format!("\"{key}\"")), "{key}");
    }
}
