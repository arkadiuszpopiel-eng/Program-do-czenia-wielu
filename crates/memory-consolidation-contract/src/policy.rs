//! Kiedy Strażniczka może pracować (PLAN §10: nie na baterii, nie w trybie gry/pełnego ekranu;
//! ACCEPTANCE F7-05). Sprawdzane przed startem i między zakresami (przerwanie przebiegu).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::{ConsolidationConfig, Trigger, in_window};
use crate::ports::HostState;

/// Powód pominięcia/przerwania przebiegu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// Konsolidacja wyłączona.
    Disabled,
    /// Zasilanie z baterii.
    OnBattery,
    /// Tryb gry / pełny ekran.
    Fullscreen,
    /// Poza oknem czasowym (harmonogram).
    OutsideWindow,
    /// Użytkownik aktywny (harmonogram).
    UserActive,
    /// Inny przebieg trwa.
    AlreadyRunning,
}

impl SkipReason {
    /// Opis dla UI (PL).
    pub fn describe(self) -> &'static str {
        match self {
            SkipReason::Disabled => "konsolidacja wyłączona w ustawieniach",
            SkipReason::OnBattery => "komputer pracuje na baterii",
            SkipReason::Fullscreen => "aktywny tryb gry lub pełny ekran",
            SkipReason::OutsideWindow => "poza oknem nocnym",
            SkipReason::UserActive => "użytkownik jest aktywny",
            SkipReason::AlreadyRunning => "konsolidacja już trwa",
        }
    }
}

/// Czy przebieg może wystartować (i trwać). Bateria i tryb gry blokują **każdy** wyzwalacz,
/// także ręczny; okno i bezczynność dotyczą tylko harmonogramu.
pub fn may_start(
    cfg: &ConsolidationConfig,
    host: &HostState,
    trigger: Trigger,
) -> Result<(), SkipReason> {
    if !cfg.enabled {
        return Err(SkipReason::Disabled);
    }
    if cfg.not_on_battery && host.on_battery {
        return Err(SkipReason::OnBattery);
    }
    if cfg.not_in_fullscreen && host.fullscreen {
        return Err(SkipReason::Fullscreen);
    }
    if trigger == Trigger::Scheduled {
        if !in_window(cfg.window_start, cfg.window_end, host.local_time) {
            return Err(SkipReason::OutsideWindow);
        }
        if host.idle_secs < cfg.min_idle_secs {
            return Err(SkipReason::UserActive);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveTime;

    use super::*;

    fn host(battery: bool, fullscreen: bool, idle: u64, h: u32) -> HostState {
        HostState {
            on_battery: battery,
            fullscreen,
            idle_secs: idle,
            local_time: NaiveTime::from_hms_opt(h, 30, 0).unwrap_or_default(),
        }
    }

    #[test]
    fn battery_and_fullscreen_block_every_trigger() {
        let cfg = ConsolidationConfig::default();
        for trigger in [Trigger::Scheduled, Trigger::Manual] {
            assert_eq!(
                may_start(&cfg, &host(true, false, 9999, 3), trigger),
                Err(SkipReason::OnBattery)
            );
            assert_eq!(
                may_start(&cfg, &host(false, true, 9999, 3), trigger),
                Err(SkipReason::Fullscreen)
            );
        }
        assert_eq!(
            may_start(&cfg, &host(false, false, 9999, 3), Trigger::Scheduled),
            Ok(())
        );
        assert_eq!(
            may_start(&cfg, &host(false, false, 10, 3), Trigger::Scheduled),
            Err(SkipReason::UserActive)
        );
        assert_eq!(
            may_start(&cfg, &host(false, false, 9999, 12), Trigger::Scheduled),
            Err(SkipReason::OutsideWindow)
        );
        assert_eq!(
            may_start(&cfg, &host(false, false, 0, 12), Trigger::Manual),
            Ok(())
        );
        let off = ConsolidationConfig {
            enabled: false,
            ..cfg.clone()
        };
        assert_eq!(
            may_start(&off, &host(false, false, 9999, 3), Trigger::Manual),
            Err(SkipReason::Disabled)
        );
        let lax = ConsolidationConfig {
            not_on_battery: false,
            ..cfg
        };
        assert_eq!(
            may_start(&lax, &host(true, false, 9999, 3), Trigger::Scheduled),
            Ok(())
        );
        assert!(!SkipReason::OnBattery.describe().is_empty());
    }
}
