//! Ustawienia głosu rozszerzonego w `core-config` (warstwa wspólna, zakres globalny). Klucze są
//! pomocnicze (poza drzewem ustawień) — zmienia je tylko panel/strona Głos przez komendy
//! `voice_*`, bo włączenie słów wywoławczych wymaga jawnego potwierdzenia ryzyka.

use app_api::dto::DictationProfile;
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, Origin, Scope};
use serde_json::Value;

/// Słowa wywoławcze włączone (domyślnie nie).
pub const WAKE_ENABLED: &str = "voice.wake_words";
/// Ryzyko nieskalibrowanego modelu przyjęte świadomie.
pub const WAKE_ACCEPT_RISK: &str = "voice.wake_accept_risk";
/// Bramka właściciela słów wywoławczych (domyślnie tak).
pub const WAKE_OWNER_GATE: &str = "voice.wake_owner_gate";
/// Weryfikacja głosu dla akcji ryzykownych (domyślnie tak).
pub const SPEAKER_REQUIRED: &str = "voice.speaker_required";
/// Tempo czytania (0,5–2,0).
pub const READ_RATE: &str = "voice.read_rate";
/// Profile dyktowania per aplikacja (lista).
pub const DICTATION_PROFILES: &str = "voice.dictation_profiles";

/// Najwięcej profili dyktowania.
pub const MAX_PROFILES: usize = 32;

/// Ustawienia.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Słowa wywoławcze włączone.
    pub wake_enabled: bool,
    /// Ryzyko przyjęte (model bez pomiaru na korpusie).
    pub wake_accept_risk: bool,
    /// Bramka właściciela.
    pub wake_owner_gate: bool,
    /// Weryfikacja głosu dla akcji ryzykownych.
    pub speaker_required: bool,
    /// Tempo czytania.
    pub read_rate: f32,
    /// Profile dyktowania.
    pub profiles: Vec<DictationProfile>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            wake_enabled: false,
            wake_accept_risk: false,
            wake_owner_gate: true,
            speaker_required: true,
            read_rate: 1.0,
            profiles: Vec::new(),
        }
    }
}

async fn get(config: &dyn ConfigStore, key: &str) -> Option<Value> {
    let key = ConfigKey::new(key).ok()?;
    config.get(&key, &Scope::Global).await.ok().flatten()
}

/// Wczytuje ustawienia (brak magazynu / wartości — domyślne).
pub async fn load(config: Option<&dyn ConfigStore>) -> Settings {
    let mut s = Settings::default();
    let Some(config) = config else {
        return s;
    };
    let flag = |v: Option<Value>, d: bool| v.and_then(|v| v.as_bool()).unwrap_or(d);
    s.wake_enabled = flag(get(config, WAKE_ENABLED).await, s.wake_enabled);
    s.wake_accept_risk = flag(get(config, WAKE_ACCEPT_RISK).await, s.wake_accept_risk);
    s.wake_owner_gate = flag(get(config, WAKE_OWNER_GATE).await, s.wake_owner_gate);
    s.speaker_required = flag(get(config, SPEAKER_REQUIRED).await, s.speaker_required);
    if let Some(rate) = get(config, READ_RATE).await.and_then(|v| v.as_f64()) {
        s.read_rate = clamp_rate(rate as f32);
    }
    if let Some(list) = get(config, DICTATION_PROFILES).await {
        s.profiles = serde_json::from_value::<Vec<DictationProfile>>(list)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| normalize_profile(p).ok())
            .take(MAX_PROFILES)
            .collect();
    }
    s
}

/// Zapis jednego klucza (błąd magazynu — komunikat dla UI).
pub async fn save(config: Option<&dyn ConfigStore>, key: &str, value: Value) -> Result<(), String> {
    let Some(config) = config else {
        return Ok(());
    };
    let key = ConfigKey::new(key).map_err(|e| e.to_string())?;
    config
        .set(
            &key,
            Some(value),
            &Scope::Global,
            &ConfigLayer::Shared,
            Origin::User,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Tempo czytania w granicach kontraktu (0,5–2,0); NaN → 1,0.
pub fn clamp_rate(rate: f32) -> f32 {
    if rate.is_finite() {
        rate.clamp(0.5, 2.0)
    } else {
        1.0
    }
}

/// Profil: nazwa pliku procesu (małe litery, bez ścieżki), 1–64 znaki, rozszerzenie `.exe`.
pub fn normalize_profile(mut p: DictationProfile) -> Result<DictationProfile, String> {
    let name = platform_contract::image_file_name(&p.app).to_lowercase();
    let ok = !name.is_empty()
        && name.chars().count() <= 64
        && name.ends_with(".exe")
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' '));
    if !ok {
        return Err(format!(
            "Profil dyktowania: „{}” nie jest nazwą programu (np. notepad.exe).",
            p.app
        ));
    }
    p.app = name;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_and_rates_are_normalized() {
        let p = normalize_profile(DictationProfile {
            app: r"C:\Windows\NOTEPAD.EXE".into(),
            capitalize_start: false,
            block_enter: true,
        })
        .unwrap();
        assert_eq!(p.app, "notepad.exe");
        for bad in ["", "notepad", "x.dll", "a<b>.exe"] {
            let r = normalize_profile(DictationProfile {
                app: bad.into(),
                capitalize_start: true,
                block_enter: false,
            });
            assert!(r.is_err(), "{bad}");
        }
        assert!((clamp_rate(5.0) - 2.0).abs() < f32::EPSILON);
        assert!((clamp_rate(f32::NAN) - 1.0).abs() < f32::EPSILON);
    }
}
