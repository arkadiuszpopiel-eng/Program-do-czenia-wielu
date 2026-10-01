//! Klasyfikacja: sygnały z okna czasu → wykrycia wg katalogu awarii (klasteryzacja po celu,
//! progi powtórzeń, przyczyna szczegółowa wygrywa z ogólną).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::FailureKind;
use crate::signal::{ModuleCondition, Resource, Signal, Symptom, TimedSignal, WatchdogSignal};

/// Progi klasyfikatora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClassifierConfig {
    /// Okno analizy (ms).
    pub window_ms: u64,
    /// Ile odpowiedzi 429 w oknie = pętla.
    pub rate_limit_hits: usize,
    /// Ile odpowiedzi 401/403 = klucz unieważniony.
    pub auth_failures: usize,
    /// Ile restartów/wyjść sidecara w oknie = pętla awarii.
    pub sidecar_restarts: usize,
    /// Ile próbek ponad budżetem = przekroczenie.
    pub resource_samples: usize,
    /// Ile `SQLITE_BUSY` w oknie = baza zablokowana.
    pub db_busy_hits: usize,
    /// Ile błędów blokady pliku w oknie = plik zablokowany.
    pub lock_hits: usize,
    /// Ile błędów sieci w oknie = brak sieci.
    pub network_hits: usize,
    /// Przesunięcie zegara uznane za awarię (ms).
    pub clock_skew_ms: i64,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            window_ms: 10 * 60 * 1000,
            rate_limit_hits: 3,
            auth_failures: 1,
            sidecar_restarts: 3,
            resource_samples: 3,
            db_busy_hits: 3,
            lock_hits: 2,
            network_hits: 2,
            clock_skew_ms: 2 * 60 * 1000,
        }
    }
}

/// Wykrycie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Detection {
    /// Rodzaj.
    pub kind: FailureKind,
    /// Cel (plik, trasa, model, moduł, wolumin…).
    pub target: String,
    /// Moduł.
    pub module: String,
    /// Dowody (po polsku, bez treści użytkownika).
    pub evidence: Vec<String>,
    /// Liczba sygnałów.
    pub count: usize,
    /// Pierwszy sygnał (ms).
    pub first_ms: u64,
    /// Ostatni sygnał (ms).
    pub last_ms: u64,
    /// Szczegóły (z sygnałów; późniejsze nadpisują).
    pub details: BTreeMap<String, String>,
}

/// Wykrycie ogólne (wyjaśniane przez szczegółową przyczynę tego samego modułu).
fn is_generic(kind: FailureKind) -> bool {
    matches!(
        kind,
        FailureKind::ModuleStartFailure | FailureKind::SidecarCrashLoop
    )
}

/// Mapowanie sygnału na (rodzaj, cel, wymagana liczba w oknie, dowód).
fn map(signal: &Signal, cfg: &ClassifierConfig) -> Option<(FailureKind, String, usize, String)> {
    let one = |k, t: String, e: String| Some((k, t, 1, e));
    match signal {
        Signal::ModuleState {
            module,
            condition: ModuleCondition::Failed { restarts },
            detail,
        } => one(
            FailureKind::ModuleStartFailure,
            module.clone(),
            format!("moduł {module} nie startuje ({restarts} prób): {detail}"),
        ),
        Signal::Watchdog(WatchdogSignal::Restart { role, .. }) => {
            let m = crate::signal::role_module(role);
            Some((
                FailureKind::SidecarCrashLoop,
                m.clone(),
                cfg.sidecar_restarts,
                format!("restart {m}"),
            ))
        }
        Signal::Watchdog(WatchdogSignal::CrashLoop { role }) => {
            let m = crate::signal::role_module(role);
            one(
                FailureKind::SidecarCrashLoop,
                m.clone(),
                format!("pętla awarii {m}"),
            )
        }
        Signal::Resource {
            scope,
            resource: Resource::DiskFreeMb,
            used,
            limit,
            ..
        } if used < limit => one(
            FailureKind::DiskFull,
            scope.clone(),
            format!("wolne {used} MB < {limit} MB na {scope}"),
        ),
        Signal::Resource {
            scope,
            resource,
            used,
            limit,
            ..
        } if *resource != Resource::DiskFreeMb && used > limit => Some((
            FailureKind::ResourceBudgetExceeded,
            scope.clone(),
            cfg.resource_samples,
            format!("{resource:?} {used} > {limit}"),
        )),
        Signal::Error {
            module,
            symptom,
            target,
            ..
        } => {
            let t = target.clone().unwrap_or_else(|| module.clone());
            let (kind, need, why) = match symptom {
                Symptom::ConfigInvalid => (
                    FailureKind::ConfigCorrupted,
                    1,
                    "plik konfiguracji niepoprawny".to_owned(),
                ),
                Symptom::KernelConfigChanged => (
                    FailureKind::KernelConfigTampered,
                    1,
                    "polityka Jądra zmieniona poza Brokerem".into(),
                ),
                Symptom::DbCorrupt => {
                    (FailureKind::SessionDbCorrupted, 1, "baza uszkodzona".into())
                }
                Symptom::DbBusy => (
                    FailureKind::SessionDbLocked,
                    cfg.db_busy_hits,
                    "baza zajęta".into(),
                ),
                Symptom::Io { os_error: 32 | 33 } => (
                    FailureKind::FileLocked,
                    cfg.lock_hits,
                    "plik zablokowany".into(),
                ),
                Symptom::Io { os_error: 5 } => {
                    (FailureKind::DirPermissionDenied, 1, "brak dostępu".into())
                }
                Symptom::Io { os_error: 39 | 112 } => {
                    (FailureKind::DiskFull, 1, "brak miejsca (we/wy)".into())
                }
                Symptom::Http { status: 401 | 403 } => (
                    FailureKind::ApiKeyRevoked,
                    cfg.auth_failures,
                    "odmowa autoryzacji".into(),
                ),
                Symptom::Http { status: 429 } => (
                    FailureKind::RateLimitLoop,
                    cfg.rate_limit_hits,
                    "429 od dostawcy".into(),
                ),
                Symptom::BudgetExceeded => {
                    (FailureKind::BudgetExhausted, 1, "limit kosztów".into())
                }
                Symptom::PortInUse { port } => {
                    (FailureKind::PortInUse, 1, format!("port {port} zajęty"))
                }
                Symptom::ModelMissing => (FailureKind::ModelMissing, 1, "brak pliku modelu".into()),
                Symptom::ModelHashMismatch => (
                    FailureKind::ModelCorrupted,
                    1,
                    "hash modelu niezgodny".into(),
                ),
                Symptom::GpuDeviceLost => (FailureKind::GpuLost, 1, "utrata urządzenia GPU".into()),
                Symptom::WebViewMissing | Symptom::WebViewCrashed => {
                    (FailureKind::WebViewBroken, 1, "WebView2".into())
                }
                Symptom::SchemaVersion { found, supported } => (
                    FailureKind::SchemaMismatch,
                    1,
                    format!("schemat {found} ≠ {supported}"),
                ),
                Symptom::UndoJournalFull => (
                    FailureKind::UndoJournalFull,
                    1,
                    "dziennik cofania pełny".into(),
                ),
                Symptom::ClockSkew { skew_ms } if skew_ms.abs() >= cfg.clock_skew_ms => (
                    FailureKind::ClockSkew,
                    1,
                    format!("zegar przesunięty o {skew_ms} ms"),
                ),
                Symptom::NetworkUnreachable => (
                    FailureKind::NetworkDown,
                    cfg.network_hits,
                    "sieć niedostępna".into(),
                ),
                Symptom::UpdateSignatureInvalid => (
                    FailureKind::UpdatePackageCorrupted,
                    1,
                    "podpis/hash paczki niezgodny".into(),
                ),
                Symptom::LogDiskLimit => (FailureKind::LogDiskLimit, 1, "limit dysku logów".into()),
                Symptom::SidecarExited { exit_code } => (
                    FailureKind::SidecarCrashLoop,
                    cfg.sidecar_restarts,
                    format!("wyjście z kodem {exit_code}"),
                ),
                Symptom::Io { .. } | Symptom::Http { .. } | Symptom::ClockSkew { .. } => {
                    return None;
                }
            };
            let t = match kind {
                FailureKind::ClockSkew => "system".into(),
                FailureKind::NetworkDown => "network".into(),
                FailureKind::SidecarCrashLoop => module.clone(),
                _ => t,
            };
            Some((kind, t, need.max(1), why))
        }
        _ => None,
    }
}

/// Dane symptomu jako szczegóły dla planisty.
fn symptom_details(symptom: &Symptom) -> Vec<(String, String)> {
    match symptom {
        Symptom::ClockSkew { skew_ms } => vec![("skew_ms".into(), skew_ms.to_string())],
        Symptom::PortInUse { port } => vec![("port".into(), port.to_string())],
        Symptom::SchemaVersion { found, supported } => vec![
            ("found".into(), found.to_string()),
            ("supported".into(), supported.to_string()),
        ],
        Symptom::SidecarExited { exit_code } => vec![("exit_code".into(), exit_code.to_string())],
        Symptom::WebViewMissing => vec![("runtime_missing".into(), "true".into())],
        _ => Vec::new(),
    }
}

/// Klasyfikuje sygnały z okna `[now − window, now]`.
pub fn classify(signals: &[TimedSignal], now_ms: u64, cfg: &ClassifierConfig) -> Vec<Detection> {
    let from = now_ms.saturating_sub(cfg.window_ms);
    let mut groups: BTreeMap<(FailureKind, String), (usize, Detection)> = BTreeMap::new();
    for s in signals
        .iter()
        .filter(|s| s.ts_ms >= from && s.ts_ms <= now_ms)
    {
        let Some((kind, target, need, why)) = map(&s.signal, cfg) else {
            continue;
        };
        let entry = groups.entry((kind, target.clone())).or_insert_with(|| {
            (
                need,
                Detection {
                    kind,
                    target,
                    module: s.signal.module(),
                    evidence: Vec::new(),
                    count: 0,
                    first_ms: s.ts_ms,
                    last_ms: s.ts_ms,
                    details: BTreeMap::new(),
                },
            )
        });
        let d = &mut entry.1;
        d.count += 1;
        d.first_ms = d.first_ms.min(s.ts_ms);
        d.last_ms = d.last_ms.max(s.ts_ms);
        if d.evidence.len() < 8 {
            d.evidence.push(why);
        }
        if let Signal::Error { details, .. } | Signal::Resource { details, .. } = &s.signal {
            d.details.extend(details.clone());
        }
        if let Signal::Error { symptom, .. } = &s.signal {
            d.details.extend(symptom_details(symptom));
        }
    }
    let found: Vec<Detection> = groups
        .into_values()
        .filter(|(need, d)| d.count >= *need)
        .map(|(_, d)| d)
        .collect();
    // Przyczyna szczegółowa (np. GPU, port, model) wyjaśnia ogólną awarię tego samego modułu.
    let specific: Vec<String> = found
        .iter()
        .filter(|d| !is_generic(d.kind))
        .map(|d| d.module.clone())
        .collect();
    found
        .into_iter()
        .filter(|d| !(is_generic(d.kind) && specific.contains(&d.module)))
        .collect()
}

#[cfg(test)]
mod tests {
    use watchdog_contract::ProcessRole;

    use super::*;

    fn err(ts: u64, module: &str, symptom: Symptom, target: &str) -> TimedSignal {
        TimedSignal {
            ts_ms: ts,
            signal: Signal::Error {
                module: module.into(),
                symptom,
                target: Some(target.into()),
                details: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn thresholds_windows_and_root_cause() {
        let cfg = ClassifierConfig::default();
        let mut s = vec![
            err(
                1_000,
                "providers-api",
                Symptom::Http { status: 429 },
                "anthropic",
            ),
            err(
                2_000,
                "providers-api",
                Symptom::Http { status: 429 },
                "anthropic",
            ),
        ];
        assert!(classify(&s, 3_000, &cfg).is_empty());
        s.push(err(
            2_500,
            "providers-api",
            Symptom::Http { status: 429 },
            "anthropic",
        ));
        let d = classify(&s, 3_000, &cfg);
        assert_eq!(
            (d.len(), d[0].kind, d[0].count),
            (1, FailureKind::RateLimitLoop, 3)
        );
        assert!(
            classify(&s, 3_000 + cfg.window_ms, &cfg).is_empty(),
            "poza oknem"
        );

        let role = ProcessRole::Sidecar("voice-stt".into());
        let mut s: Vec<TimedSignal> = (0..3)
            .map(|i| TimedSignal {
                ts_ms: i,
                signal: Signal::Watchdog(WatchdogSignal::Restart {
                    role: role.clone(),
                    attempt: 1,
                }),
            })
            .collect();
        assert_eq!(
            classify(&s, 10, &cfg)[0].kind,
            FailureKind::SidecarCrashLoop
        );
        s.push(err(5, "voice-stt", Symptom::GpuDeviceLost, "voice-stt"));
        let d = classify(&s, 10, &cfg);
        assert_eq!(
            d.iter().map(|d| d.kind).collect::<Vec<_>>(),
            [FailureKind::GpuLost]
        );

        let small = vec![
            err(1, "core", Symptom::ClockSkew { skew_ms: 1_000 }, "x"),
            err(1, "fs", Symptom::Io { os_error: 2 }, "x"),
        ];
        assert!(classify(&small, 2, &cfg).is_empty());
    }
}
