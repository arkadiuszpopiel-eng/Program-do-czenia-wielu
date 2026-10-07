//! Sygnały wejściowe Diagnosty: stan modułów (`core-registry`), błędy (`core-log`, strumień
//! Diagnostyka), akcje watchdoga, pomiary zasobów — z konwersją ze zdarzeń magistrali.

use std::collections::BTreeMap;

use core_bus_contract::{Event, EventKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use watchdog_contract::{ProcessRole, WatchAction};

/// Rodzaj zdarzenia, którym moduły zgłaszają symptomy Diagnoście (strumień Diagnostyka):
/// ładunek `{module, symptom, target?, details?}` (pola jak w [`Signal::Error`]).
pub const EVENT_SYMPTOM: &str = "diagnostics.symptom";

/// Stan modułu (z rejestru).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "condition", rename_all = "snake_case")]
pub enum ModuleCondition {
    /// Działa.
    Ready,
    /// Działa z ograniczeniami.
    Degraded,
    /// Start nie powiódł się.
    Failed {
        /// Liczba nieudanych prób.
        restarts: u8,
    },
    /// Wyłączony.
    Disabled,
    /// Niezaładowany.
    Unloaded,
}

/// Zasób mierzony.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    /// RAM (MB).
    RamMb,
    /// CPU (%).
    CpuPct,
    /// Wolne miejsce na dysku (MB) — `used` = wolne, `limit` = minimum.
    DiskFreeMb,
    /// VRAM (MB).
    VramMb,
}

/// Symptom zgłaszany przez moduł (stabilne kody, niezależne od treści komunikatów).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Symptom {
    /// Plik konfiguracji nie przeszedł walidacji (`config.invalid`).
    ConfigInvalid,
    /// Zmiana `kernel.*` w pliku poza Brokerem.
    KernelConfigChanged,
    /// SQLite: baza uszkodzona (`SQLITE_CORRUPT`/`NOTADB`).
    DbCorrupt,
    /// SQLite: baza zajęta (`SQLITE_BUSY`/`LOCKED`).
    DbBusy,
    /// Błąd we/wy z kodem systemu (32/33 blokada, 5 brak dostępu, 112/39 brak miejsca).
    Io {
        /// Kod błędu systemu.
        os_error: i32,
    },
    /// Odpowiedź HTTP dostawcy.
    Http {
        /// Status.
        status: u16,
    },
    /// Odmowa z powodu limitu kosztów (`cost-meter`, tryb Enforced).
    BudgetExceeded,
    /// Port zajęty (WSAEADDRINUSE).
    PortInUse {
        /// Port.
        port: u16,
    },
    /// Brak pliku modelu.
    ModelMissing,
    /// Hash modelu niezgodny.
    ModelHashMismatch,
    /// Utrata urządzenia GPU (DXGI device removed / Vulkan device lost / CUDA).
    GpuDeviceLost,
    /// Brak środowiska WebView2.
    WebViewMissing,
    /// Proces WebView2 się wysypał.
    WebViewCrashed,
    /// Wersja schematu danych niezgodna z kodem.
    SchemaVersion {
        /// Wersja w danych.
        found: u32,
        /// Wersja obsługiwana.
        supported: u32,
    },
    /// Dziennik cofania pełny.
    UndoJournalFull,
    /// Przesunięcie zegara (np. z błędów TLS / porównania z NTP).
    ClockSkew {
        /// Przesunięcie (ms).
        skew_ms: i64,
    },
    /// Sieć niedostępna.
    NetworkUnreachable,
    /// Paczka aktualizacji: zły podpis albo hash.
    UpdateSignatureInvalid,
    /// Strumień logów osiągnął limit dysku (`LogError::DiskLimit`).
    LogDiskLimit,
    /// Proces pomocniczy zakończył się z kodem.
    SidecarExited {
        /// Kod wyjścia.
        exit_code: i32,
    },
}

/// Sygnał watchdoga.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "watchdog", rename_all = "snake_case")]
pub enum WatchdogSignal {
    /// Restart procesu.
    Restart {
        /// Proces.
        role: ProcessRole,
        /// Numer w oknie.
        attempt: u8,
    },
    /// Pętla awarii.
    CrashLoop {
        /// Proces.
        role: ProcessRole,
    },
    /// Wejście w safe-mode.
    SafeModeEntered {
        /// Powód.
        reason: String,
    },
    /// Wyjście z safe-mode.
    SafeModeLeft,
}

/// Sygnał.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "signal", rename_all = "snake_case")]
pub enum Signal {
    /// Stan modułu.
    ModuleState {
        /// Moduł.
        module: String,
        /// Stan.
        condition: ModuleCondition,
        /// Opis.
        #[serde(default)]
        detail: String,
    },
    /// Symptom błędu.
    Error {
        /// Moduł zgłaszający.
        module: String,
        /// Symptom.
        symptom: Symptom,
        /// Cel (plik, trasa, model, katalog…).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        /// Szczegóły dla planisty (np. `device_key`, `port_key`, `sha256`).
        #[serde(default)]
        details: BTreeMap<String, String>,
    },
    /// Watchdog.
    Watchdog(WatchdogSignal),
    /// Pomiar zasobu.
    Resource {
        /// Moduł albo `system` / wolumin.
        scope: String,
        /// Zasób.
        resource: Resource,
        /// Wartość.
        used: u64,
        /// Limit (budżet, minimum wolnego miejsca).
        limit: u64,
        /// Szczegóły.
        #[serde(default)]
        details: BTreeMap<String, String>,
    },
}

/// Sygnał ze znacznikiem czasu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TimedSignal {
    /// Czas (ms).
    pub ts_ms: u64,
    /// Sygnał.
    pub signal: Signal,
}

/// Nazwa modułu dla roli procesu.
pub fn role_module(role: &ProcessRole) -> String {
    match role {
        ProcessRole::Sidecar(id) | ProcessRole::CliBridge(id) | ProcessRole::Tool(id) => id.clone(),
        ProcessRole::Core => "core".into(),
        ProcessRole::BrokerUi => "broker-ui".into(),
        ProcessRole::Broker => "safety-broker".into(),
    }
}

fn text(v: &Value, field: &str) -> Option<String> {
    v.get(field).and_then(Value::as_str).map(str::to_owned)
}

fn condition(name: &str, restarts: u8) -> Option<ModuleCondition> {
    Some(match name {
        "ready" | "healthy" => ModuleCondition::Ready,
        "degraded" => ModuleCondition::Degraded,
        "failed" | "unhealthy" => ModuleCondition::Failed { restarts },
        "disabled" => ModuleCondition::Disabled,
        "unloaded" | "not_started" => ModuleCondition::Unloaded,
        _ => return None,
    })
}

impl Signal {
    /// Sygnał z akcji watchdoga (dziennik akcji, gdy brak magistrali).
    pub fn from_watch_action(action: &WatchAction) -> Option<Signal> {
        Some(Signal::Watchdog(match action {
            WatchAction::Restart { role, attempt } => WatchdogSignal::Restart {
                role: role.clone(),
                attempt: *attempt,
            },
            WatchAction::EnterSafeMode { reason } => WatchdogSignal::SafeModeEntered {
                reason: reason.clone(),
            },
            WatchAction::LeaveSafeMode => WatchdogSignal::SafeModeLeft,
            _ => return None,
        }))
    }

    /// Sygnał ze zdarzenia magistrali (rejestr, watchdog, `config.invalid`, [`EVENT_SYMPTOM`]).
    /// Zdarzenia nierozpoznane → `None`.
    pub fn from_event(event: &Event) -> Option<Signal> {
        let EventKind::Custom(name) = &event.kind else {
            return None;
        };
        let p = &event.payload;
        let role = || serde_json::from_value::<ProcessRole>(p.get("role")?.clone()).ok();
        match name.as_str() {
            "registry.module.state_changed" => Some(Signal::ModuleState {
                module: text(p, "module")?,
                condition: condition(&text(p, "to")?, 1)?,
                detail: text(p, "reason").unwrap_or_default(),
            }),
            "registry.module.health" => Some(Signal::ModuleState {
                module: text(p, "module")?,
                condition: condition(&text(p, "status")?, 0)?,
                detail: text(p, "detail").unwrap_or_default(),
            }),
            "watchdog.restart" => Some(Signal::Watchdog(WatchdogSignal::Restart {
                role: role()?,
                attempt: p
                    .get("attempt")
                    .and_then(Value::as_u64)
                    .and_then(|a| u8::try_from(a).ok())
                    .unwrap_or(1),
            })),
            "watchdog.crash_loop" => Some(Signal::Watchdog(WatchdogSignal::CrashLoop {
                role: role()?,
            })),
            "watchdog.safe_mode.entered" => {
                Some(Signal::Watchdog(WatchdogSignal::SafeModeEntered {
                    reason: text(p, "reason").unwrap_or_default(),
                }))
            }
            "watchdog.safe_mode.left" => Some(Signal::Watchdog(WatchdogSignal::SafeModeLeft)),
            "config.invalid" => {
                let reason = text(p, "reason").unwrap_or_default();
                let symptom = if reason.contains("kernel") {
                    Symptom::KernelConfigChanged
                } else {
                    Symptom::ConfigInvalid
                };
                Some(Signal::Error {
                    module: "core-config".into(),
                    symptom,
                    target: text(p, "path").or_else(|| text(p, "file")),
                    details: BTreeMap::from([("reason".to_owned(), reason)]),
                })
            }
            EVENT_SYMPTOM => serde_json::from_value(serde_json::json!({
                "signal": "error",
                "module": p.get("module")?,
                "symptom": p.get("symptom")?,
                "target": p.get("target"),
                "details": p.get("details").cloned().unwrap_or(Value::Object(Default::default())),
            }))
            .ok(),
            _ => None,
        }
    }

    /// Moduł, którego dotyczy sygnał.
    pub fn module(&self) -> String {
        match self {
            Signal::ModuleState { module, .. } | Signal::Error { module, .. } => module.clone(),
            Signal::Resource { scope, .. } => scope.clone(),
            Signal::Watchdog(
                WatchdogSignal::Restart { role, .. } | WatchdogSignal::CrashLoop { role },
            ) => role_module(role),
            Signal::Watchdog(_) => "watchdog".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use core_bus_contract::Level;
    use serde_json::json;

    use super::*;

    fn ev(name: &str, payload: Value) -> Event {
        Event::new(EventKind::Custom(name.into()), Level::Warn, payload)
    }

    #[test]
    fn events_become_signals() {
        let s = Signal::from_event(&ev(
            "registry.module.state_changed",
            json!({"module": "voice-stt", "from": "loading", "to": "failed", "reason": "brak"}),
        ))
        .unwrap();
        assert!(
            matches!(s, Signal::ModuleState { ref module, condition: ModuleCondition::Failed { .. }, .. } if module == "voice-stt")
        );
        let s = Signal::from_event(&ev(
            "watchdog.restart",
            json!({"role": {"role": "sidecar", "id": "voice-stt"}, "attempt": 2}),
        ))
        .unwrap();
        assert_eq!(s.module(), "voice-stt");
        let s = Signal::from_event(&ev(EVENT_SYMPTOM, json!({"module": "providers-api", "symptom": {"code": "http", "status": 429}, "target": "anthropic"}))).unwrap();
        assert!(matches!(
            s,
            Signal::Error {
                symptom: Symptom::Http { status: 429 },
                ..
            }
        ));
        let s = Signal::from_event(&ev(
            "config.invalid",
            json!({"reason": "zmiana kernel.egress poza Brokerem"}),
        ))
        .unwrap();
        assert!(matches!(
            s,
            Signal::Error {
                symptom: Symptom::KernelConfigChanged,
                ..
            }
        ));
        assert!(Signal::from_event(&ev("inne.zdarzenie", json!({}))).is_none());
        assert!(
            Signal::from_event(&Event::new(EventKind::Voice, Level::Info, json!({}))).is_none()
        );
        let a = WatchAction::EnterSafeMode {
            reason: "pętla".into(),
        };
        assert!(matches!(
            Signal::from_watch_action(&a),
            Some(Signal::Watchdog(WatchdogSignal::SafeModeEntered { .. }))
        ));
        assert!(
            Signal::from_watch_action(&WatchAction::RollbackSkipped {
                reason: String::new()
            })
            .is_none()
        );
        let json = serde_json::to_value(Signal::Watchdog(WatchdogSignal::SafeModeLeft)).unwrap();
        assert_eq!(
            serde_json::from_value::<Signal>(json).unwrap(),
            Signal::Watchdog(WatchdogSignal::SafeModeLeft)
        );
    }
}
