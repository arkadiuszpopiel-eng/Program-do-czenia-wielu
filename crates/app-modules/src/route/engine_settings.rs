//! Ustawienia silnika rozmowy (Ustawienia → Modele i silniki, klucze `engines.llm.*`, nakładka
//! maszyny): backend `llama-server`, najdłuższy kontekst, wątki procesora, zwalnianie modelu po
//! bezczynności. Czytane przy budowie modułu `providers-local` — zmiana działa od następnego
//! uruchomienia Alfy. Wartość nieznana albo spoza zakresu = ustawienie domyślne (bez błędu).

use std::time::Duration;

use device_profile_contract::Backend;
use providers_local_impl::{BackendChoice, LocalConfig};
use serde_json::Value;

/// Klucze ustawień (`app-core/data/settings-pages.json`, strona „Modele i silniki”).
pub mod keys {
    /// `auto` | `cuda` | `vulkan` | `cpu`.
    pub const BACKEND: &str = "engines.llm.backend";
    /// `auto` | `4096` | `2048` (tokeny).
    pub const CONTEXT: &str = "engines.llm.context";
    /// Wątki procesora; 0 = rdzenie fizyczne.
    pub const THREADS: &str = "engines.llm.threads";
    /// Minuty bezczynności do zwolnienia modelu z pamięci.
    pub const IDLE_UNLOAD_MIN: &str = "engines.llm.idle_unload_min";
    /// Wszystkie klucze (odczyt przy starcie).
    pub const ALL: [&str; 4] = [BACKEND, CONTEXT, THREADS, IDLE_UNLOAD_MIN];
}

/// Najwięcej wątków przyjmowanych z ustawień.
pub const MAX_THREADS: u32 = 64;
/// Zakres zwalniania po bezczynności (minuty).
pub const IDLE_RANGE_MIN: (u32, u32) = (1, 120);

/// Ustawienia silnika rozmowy; `None` — wartość domyślna `LocalConfig`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LlmSettings {
    /// Wymuszony backend (`None` — według profilu urządzenia).
    pub backend: Option<Backend>,
    /// Najdłuższy kontekst w tokenach (`None` — z manifestu, na małej karcie zmniejszany).
    pub context: Option<u32>,
    /// Wątki procesora (`None` — rdzenie fizyczne).
    pub threads: Option<u32>,
    /// Zwolnienie po bezczynności (`None` — 10 minut).
    pub idle_unload_min: Option<u32>,
}

fn number(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

impl LlmSettings {
    /// Z wartości ustawień (`get(klucz)`), z odrzuceniem wartości nieznanych.
    pub fn from_values(get: impl Fn(&str) -> Option<Value>) -> Self {
        let backend = match get(keys::BACKEND).as_ref().and_then(Value::as_str) {
            Some("cuda") => Some(Backend::Cuda),
            Some("vulkan") => Some(Backend::Vulkan),
            Some("cpu") => Some(Backend::Cpu),
            _ => None,
        };
        let context = number(get(keys::CONTEXT).as_ref())
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| matches!(n, 2_048 | 4_096));
        let threads = number(get(keys::THREADS).as_ref())
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| (1..=MAX_THREADS).contains(n));
        let idle_unload_min = number(get(keys::IDLE_UNLOAD_MIN).as_ref())
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| (IDLE_RANGE_MIN.0..=IDLE_RANGE_MIN.1).contains(n));
        Self {
            backend,
            context,
            threads,
            idle_unload_min,
        }
    }

    /// Nakłada ustawienia na konfigurację dostawcy lokalnego.
    pub fn apply(&self, config: &mut LocalConfig) {
        if let Some(b) = self.backend {
            config.backend = BackendChoice::Fixed(b);
        }
        if let Some(ctx) = self.context {
            config.ctx = ctx;
        }
        if self.threads.is_some() {
            config.threads = self.threads;
        }
        if let Some(min) = self.idle_unload_min {
            config.idle_unload = Duration::from_secs(u64::from(min) * 60);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    fn read(pairs: &[(&str, Value)]) -> LlmSettings {
        let map: BTreeMap<String, Value> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        LlmSettings::from_values(|k| map.get(k).cloned())
    }

    #[test]
    fn defaults_and_unknown_values_keep_local_config() {
        let base = LocalConfig::new("m", "s");
        for settings in [
            read(&[]),
            read(&[
                (keys::BACKEND, json!("auto")),
                (keys::CONTEXT, json!("auto")),
                (keys::THREADS, json!(0)),
                (keys::IDLE_UNLOAD_MIN, json!(0)),
            ]),
            read(&[
                (keys::BACKEND, json!("rocm")),
                (keys::CONTEXT, json!("1000")),
                (keys::THREADS, json!(500)),
                (keys::IDLE_UNLOAD_MIN, json!(true)),
            ]),
        ] {
            assert_eq!(settings, LlmSettings::default());
            let mut config = base.clone();
            settings.apply(&mut config);
            assert_eq!(config, base);
        }
    }

    #[test]
    fn values_reach_the_sidecar_config() {
        let settings = read(&[
            (keys::BACKEND, json!("cpu")),
            (keys::CONTEXT, json!("2048")),
            (keys::THREADS, json!(6)),
            (keys::IDLE_UNLOAD_MIN, json!("5")),
        ]);
        let mut config = LocalConfig::new("m", "s");
        settings.apply(&mut config);
        assert_eq!(config.backend, BackendChoice::Fixed(Backend::Cpu));
        assert_eq!(
            (config.ctx, config.threads, config.idle_unload),
            (2_048, Some(6), Duration::from_secs(300))
        );
        let vulkan = read(&[
            (keys::BACKEND, json!("vulkan")),
            (keys::CONTEXT, json!(4096)),
        ]);
        assert_eq!(
            (vulkan.backend, vulkan.context),
            (Some(Backend::Vulkan), Some(4_096))
        );
    }
}
