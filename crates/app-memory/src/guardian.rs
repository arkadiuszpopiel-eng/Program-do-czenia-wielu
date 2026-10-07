//! Strażniczka pamięci w aplikacji: `ConsolidationModule` z modelem lokalnym (Router lokalny —
//! dane nie opuszczają maszyny), budżetem tła `cost-meter`, stanem maszyny z `device-profile`
//! i licznikiem bezczynności. **Licznik bezczynności:** platforma nie ma jeszcze
//! `GetLastInputInfo` w `platform-contract` — port [`IdleSource`] z atrapą „nigdy bezczynny"
//! (`UnknownIdle`): harmonogram nocny nie startuje, „Uporządkuj teraz" działa (dalej nie na
//! baterii ani w trybie gry).

use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveTime;
use core_bus_contract::{Event, EventBus, EventKind, Level};
use cost_meter_contract::CostMeter;
use device_profile_contract::DeviceProfile;
use memory_consolidation_contract::{AutoExtract, ConsolidationConfig, Guardian, GuardianPorts};
use memory_consolidation_impl::{
    ConsolidationModule, CostMeterBudget, DeviceHost, LlmConsolidator, SystemLocalClock,
};
pub use memory_consolidation_impl::{IdleSource, UnknownIdle};
use memory_contract::{EventSink, MemoryService, PrivacyOracle, SessionId, SystemClock};
use providers_contract::ModelProvider;

use app_api::AppError;

/// Klucze ustawień pamięci (`data/settings-pages.json`, strona „Pamięć").
pub mod keys {
    /// Porządkowanie nocne włączone.
    pub const CONSOLIDATION: &str = "memory.consolidation_enabled";
    /// Okno nocne `HH:MM-HH:MM`.
    pub const WINDOW: &str = "memory.consolidation_window";
    /// Ekstrakcja faktów: `ask`, `on`, `off`.
    pub const AUTO_EXTRACT: &str = "memory.auto_extract";
}

/// Domyślne okno nocne.
pub const DEFAULT_WINDOW: &str = "02:00-05:00";

/// Zdarzenia Strażniczki → magistrala (`memory.consolidation.*`, bez treści).
pub struct BusSink {
    bus: Arc<dyn EventBus>,
}

impl BusSink {
    /// Odbiorca na magistrali.
    pub fn new(bus: Arc<dyn EventBus>) -> Self {
        Self { bus }
    }
}

impl EventSink for BusSink {
    fn emit(&self, kind: &str, session: Option<&SessionId>, payload: serde_json::Value) {
        let mut event = Event::new(EventKind::Custom(kind.to_owned()), Level::Info, payload);
        if let Some(s) = session {
            event = event.with_session(s.clone());
        }
        let bus = self.bus.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                // Zdarzenie diagnostyczne — błąd magistrali nie przerywa porządkowania.
                let _ = bus.publish(event).await;
            });
        }
    }
}

fn hm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

/// Konfiguracja z Ustawień → Pamięć (wartości spoza listy → domyślne).
pub fn config(
    enabled: bool,
    auto_extract: Option<&str>,
    window: Option<&str>,
) -> ConsolidationConfig {
    let mut c = ConsolidationConfig {
        enabled,
        auto_extract: match auto_extract {
            Some("on") => AutoExtract::On,
            Some("off") => AutoExtract::Off,
            _ => AutoExtract::Ask,
        },
        ..ConsolidationConfig::default()
    };
    if let Some((a, b)) = window.and_then(|w| w.split_once('-'))
        && let (Some(start), Some(end)) = (hm(a), hm(b))
    {
        c.window_start = start;
        c.window_end = end;
    }
    c
}

/// Zależności Strażniczki.
pub struct GuardianDeps {
    /// Pamięć.
    pub memory: Arc<dyn MemoryService>,
    /// Prywatność sesji.
    pub privacy: Arc<dyn PrivacyOracle>,
    /// Stan maszyny (bateria, pełny ekran).
    pub device: Arc<dyn DeviceProfile>,
    /// Licznik bezczynności.
    pub idle: Arc<dyn IdleSource>,
    /// Budżet tła.
    pub meter: Arc<dyn CostMeter>,
    /// Model lokalny (dostawca + model); `None` — tylko reguły deterministyczne.
    pub model: Option<(Arc<dyn ModelProvider>, String)>,
    /// Magistrala (`memory.consolidation.*`).
    pub bus: Arc<dyn EventBus>,
    /// Konfiguracja.
    pub config: ConsolidationConfig,
    /// Odstęp sprawdzania harmonogramu.
    pub interval: Duration,
}

/// Moduł `memory-consolidation` złożony z portów aplikacji.
pub fn module(deps: GuardianDeps) -> Result<ConsolidationModule, AppError> {
    let consolidator = deps.model.map(|(provider, model)| {
        Arc::new(LlmConsolidator::new(provider, model, true))
            as Arc<dyn memory_consolidation_contract::Consolidator>
    });
    let ports = GuardianPorts {
        memory: deps.memory,
        consolidator,
        budget: Arc::new(CostMeterBudget::new(deps.meter)),
        host: Arc::new(DeviceHost::new(
            deps.device,
            deps.idle,
            Arc::new(SystemLocalClock),
        )),
        privacy: deps.privacy,
        events: Arc::new(BusSink::new(deps.bus)),
        clock: Arc::new(SystemClock),
    };
    ConsolidationModule::new(Guardian::new(ports, deps.config), deps.interval)
        .map_err(|e| AppError::internal(format!("memory-consolidation: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_settings() {
        let c = config(false, Some("on"), Some("23:30-04:15"));
        assert!(!c.enabled);
        assert_eq!(c.auto_extract, AutoExtract::On);
        assert_eq!(c.window_start, NaiveTime::from_hms_opt(23, 30, 0).unwrap());
        assert_eq!(c.window_end, NaiveTime::from_hms_opt(4, 15, 0).unwrap());
        let d = config(true, Some("x"), Some("zła"));
        assert_eq!(d.auto_extract, AutoExtract::Ask);
        assert_eq!(d.window_start, ConsolidationConfig::default().window_start);
        assert_eq!(UnknownIdle.idle_secs(), 0);
    }
}
