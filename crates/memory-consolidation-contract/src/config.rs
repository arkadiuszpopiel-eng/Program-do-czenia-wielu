//! Konfiguracja konsolidacji (`[memory.consolidation]`, docs/modules/memory/SPEC.md) i wyzwalacz.

use chrono::NaiveTime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Ekstrakcja faktów z epizodów (`[memory] auto_extract`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutoExtract {
    /// Fakty powstają jako oczekujące na zatwierdzenie (domyślnie).
    #[default]
    Ask,
    /// Fakty od razu aktywne (tylko w zakresie sesji; zakresy szersze zawsze za zgodą).
    On,
    /// Bez modelu językowego (tylko reguły deterministyczne).
    Off,
}

/// Co uruchomiło przebieg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// Harmonogram (okno nocne, bezczynność).
    Scheduled,
    /// Użytkownik („Uporządkuj pamięć teraz”) — bez okna i bezczynności, ale nadal nie na baterii
    /// ani w trybie gry.
    Manual,
}

/// Konfiguracja Strażniczki pamięci.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ConsolidationConfig {
    /// Włączona.
    pub enabled: bool,
    /// Początek okna (czas lokalny).
    pub window_start: NaiveTime,
    /// Koniec okna (czas lokalny; okno może przechodzić przez północ).
    pub window_end: NaiveTime,
    /// Nie startuje na baterii.
    pub not_on_battery: bool,
    /// Nie startuje w trybie gry / pełnego ekranu.
    pub not_in_fullscreen: bool,
    /// Minimalna bezczynność użytkownika (s) dla przebiegu z harmonogramu.
    pub min_idle_secs: u64,
    /// Ekstrakcja faktów.
    pub auto_extract: AutoExtract,
    /// Maksymalna liczba epizodów na zakres w jednym przebiegu.
    pub max_episodes_per_scope: usize,
    /// Maksymalna liczba znanych faktów w kontekście modelu.
    pub max_known_facts: usize,
    /// Próg podobieństwa rdzeni (Jaccard) dla prawie-duplikatów (0–1; 1 = tylko identyczne).
    pub dedup_similarity: f32,
    /// Retencja epizodów już przetworzonych (dni; `None` = bez usuwania).
    pub episodic_retention_days: Option<u64>,
    /// Propozycje awansu do pamięci globalnej (fakt powtarzający się w wielu sesjach; oczekujące).
    pub propose_promotions: bool,
    /// Minimalna liczba różnych sesji dla propozycji awansu.
    pub promotion_min_sessions: usize,
    /// Sesje prywatne konsoliduje wyłącznie model lokalny.
    pub require_local_for_private: bool,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            window_start: NaiveTime::from_hms_opt(2, 0, 0).unwrap_or_default(),
            window_end: NaiveTime::from_hms_opt(5, 0, 0).unwrap_or_default(),
            not_on_battery: true,
            not_in_fullscreen: true,
            min_idle_secs: 600,
            auto_extract: AutoExtract::Ask,
            max_episodes_per_scope: 50,
            max_known_facts: 100,
            dedup_similarity: 0.9,
            episodic_retention_days: Some(180),
            propose_promotions: true,
            promotion_min_sessions: 2,
            require_local_for_private: true,
        }
    }
}

/// Czy `t` leży w oknie `[start, end)` (okno przez północ, gdy `start > end`; `start == end` =
/// cała doba).
pub fn in_window(start: NaiveTime, end: NaiveTime, t: NaiveTime) -> bool {
    match start.cmp(&end) {
        std::cmp::Ordering::Less => start <= t && t < end,
        std::cmp::Ordering::Greater => t >= start || t < end,
        std::cmp::Ordering::Equal => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap_or_default()
    }

    #[test]
    fn windows_including_midnight() {
        assert!(in_window(t(2, 0), t(5, 0), t(2, 0)));
        assert!(!in_window(t(2, 0), t(5, 0), t(5, 0)));
        assert!(in_window(t(23, 0), t(4, 0), t(0, 30)));
        assert!(in_window(t(23, 0), t(4, 0), t(23, 30)));
        assert!(!in_window(t(23, 0), t(4, 0), t(12, 0)));
        assert!(in_window(t(3, 0), t(3, 0), t(15, 0)));
        let cfg = ConsolidationConfig::default();
        assert_eq!((cfg.window_start, cfg.window_end), (t(2, 0), t(5, 0)));
    }
}
