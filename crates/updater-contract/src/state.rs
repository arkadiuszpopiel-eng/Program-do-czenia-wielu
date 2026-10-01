//! `current.json` (wersja aktywna + poprzednia) i deterministyczna logika launchera wspólna dla
//! `-impl` i `-fake`: wybór wersji, przełączenie, rollback, polityka crash-loop, sprzątanie.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::UpdaterError;

/// Wersja schematu `current.json`.
pub const STATE_SCHEMA: u32 = 1;

/// Zawartość `current.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CurrentState {
    /// Wersja schematu pliku.
    pub schema: u32,
    /// Wersja aktywna.
    #[schemars(with = "String")]
    pub active: Version,
    /// Poprzednia wersja (rollback).
    #[schemars(with = "Option<String>")]
    pub previous: Option<Version>,
    /// Aktywna czeka na pierwszy zdrowy start (`mark_good`); jedna szybka awaria → powrót.
    pub pending: bool,
    /// Kolejne szybkie awarie aktywnej wersji.
    #[serde(default)]
    pub crashes: u32,
    /// Wersje wycofane po crash-loopie (nie są wybierane ani przywracane).
    #[serde(default)]
    #[schemars(with = "Vec<String>")]
    pub bad: Vec<Version>,
    /// Ostatnia zmiana.
    pub updated_at: DateTime<Utc>,
}

impl CurrentState {
    /// Stan po instalacji pierwszej wersji (uznanej za dobrą).
    pub fn initial(active: Version, now: DateTime<Utc>) -> Self {
        Self {
            schema: STATE_SCHEMA,
            active,
            previous: None,
            pending: false,
            crashes: 0,
            bad: Vec::new(),
            updated_at: now,
        }
    }

    /// Przełączenie na `to`: dotychczasowa aktywna staje się poprzednią, nowa czeka na `mark_good`.
    pub fn switched(&self, to: &Version, now: DateTime<Utc>) -> Self {
        if *to == self.active {
            return self.clone();
        }
        Self {
            schema: STATE_SCHEMA,
            previous: Some(self.active.clone()),
            active: to.clone(),
            pending: true,
            crashes: 0,
            bad: self.bad.iter().filter(|b| *b != to).cloned().collect(),
            updated_at: now,
        }
    }

    /// Rollback = przełączenie na poprzednią (aktywna staje się poprzednią — można wrócić).
    pub fn rolled_back(&self, now: DateTime<Utc>) -> Result<Self, UpdaterError> {
        let previous = self.previous.clone().ok_or(UpdaterError::NoPrevious)?;
        Ok(Self {
            schema: STATE_SCHEMA,
            active: previous,
            previous: Some(self.active.clone()),
            pending: false,
            crashes: 0,
            bad: self.bad.clone(),
            updated_at: now,
        })
    }

    /// Po zdrowym starcie aktywnej wersji.
    pub fn marked_good(&self, version: &Version, now: DateTime<Utc>) -> Self {
        let mut next = self.clone();
        if *version == self.active && (self.pending || self.crashes > 0) {
            next.pending = false;
            next.crashes = 0;
            next.updated_at = now;
        }
        next
    }

    /// Czy wersja jest wycofana.
    pub fn is_bad(&self, version: &Version) -> bool {
        self.bad.contains(version)
    }
}

/// Wybrana wersja do uruchomienia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Choice {
    /// Wersja.
    #[schemars(with = "String")]
    pub version: Version,
    /// Czy to wersja zapasowa (aktywna niedostępna albo brak `current.json`).
    pub fallback: bool,
    /// Powód użycia wersji zapasowej.
    pub reason: Option<String>,
}

/// Wybór wersji: aktywna → (brak/uszkodzona) poprzednia → błąd. Bez `current.json` (brak albo
/// uszkodzony plik) — najnowsza poprawna zainstalowana.
pub fn choose_version(
    state: Option<&CurrentState>,
    installed: &[Version],
    usable: &dyn Fn(&Version) -> bool,
) -> Result<Choice, UpdaterError> {
    let Some(state) = state else {
        let newest = installed.iter().filter(|v| usable(v)).max().cloned();
        return newest
            .map(|version| Choice {
                version,
                fallback: true,
                reason: Some("brak lub uszkodzony current.json".to_owned()),
            })
            .ok_or_else(|| UpdaterError::NoUsableVersion {
                reason: "brak current.json i zainstalowanych wersji".to_owned(),
            });
    };
    if usable(&state.active) && !state.is_bad(&state.active) {
        return Ok(Choice {
            version: state.active.clone(),
            fallback: false,
            reason: None,
        });
    }
    match &state.previous {
        Some(prev) if usable(prev) && !state.is_bad(prev) => Ok(Choice {
            version: prev.clone(),
            fallback: true,
            reason: Some(format!(
                "wersja {} niedostępna lub uszkodzona",
                state.active
            )),
        }),
        _ => Err(UpdaterError::NoUsableVersion {
            reason: format!("aktywna {} i poprzednia niedostępne", state.active),
        }),
    }
}

/// Polityka wykrywania crash-loopu w launcherze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CrashPolicy {
    /// Okno obserwacji po starcie (ms): wyjście z kodem ≠ 0 w tym oknie = szybka awaria.
    pub window_ms: u64,
    /// Liczba kolejnych szybkich awarii, po której launcher wraca do poprzedniej wersji
    /// (dla wersji czekającej na `mark_good` wystarcza jedna).
    pub max_quick_crashes: u32,
}

impl Default for CrashPolicy {
    fn default() -> Self {
        Self {
            window_ms: 15_000,
            max_quick_crashes: 2,
        }
    }
}

/// Jak zakończyło się uruchomienie aplikacji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "exit", rename_all = "snake_case")]
pub enum AppExit {
    /// Proces działa dłużej niż okno obserwacji.
    Running,
    /// Proces zakończył się.
    Exited {
        /// Kod wyjścia.
        code: i32,
        /// Po ilu ms od startu.
        after_ms: u64,
    },
    /// Nie udało się uruchomić procesu.
    FailedToStart {
        /// Powód.
        reason: String,
    },
}

/// Decyzja launchera po zakończeniu obserwacji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ExitDecision {
    /// Start zdrowy — koniec pracy launchera.
    Healthy,
    /// Ponów start tej samej wersji.
    Retry,
    /// Crash-loop: przełączono na poprzednią wersję — uruchom ją.
    FallBack {
        /// Wersja zapasowa.
        #[schemars(with = "String")]
        to: Version,
    },
    /// Nic nie działa — pokaż błąd.
    GiveUp {
        /// Powód.
        reason: String,
    },
}

/// Nowy stan i decyzja po uruchomieniu `version`. `previous_usable` — czy poprzednia wersja
/// jest zainstalowana i poprawna.
pub fn decide_exit(
    state: &CurrentState,
    version: &Version,
    exit: &AppExit,
    policy: &CrashPolicy,
    previous_usable: bool,
    now: DateTime<Utc>,
) -> (CurrentState, ExitDecision) {
    let quick = match exit {
        AppExit::Running => false,
        AppExit::Exited { code, after_ms } => *code != 0 && *after_ms < policy.window_ms,
        AppExit::FailedToStart { .. } => true,
    };
    let mut next = state.clone();
    if !quick {
        if next.crashes > 0 && *version == state.active {
            next.crashes = 0;
            next.updated_at = now;
        }
        return (next, ExitDecision::Healthy);
    }
    next.crashes = next.crashes.saturating_add(1);
    next.updated_at = now;
    let limit_hit = next.crashes >= policy.max_quick_crashes.max(1);
    if *version != state.active {
        let decision = if limit_hit {
            ExitDecision::GiveUp {
                reason: format!("wersja zapasowa {version} też nie startuje"),
            }
        } else {
            ExitDecision::Retry
        };
        return (next, decision);
    }
    if !(state.pending || limit_hit) {
        return (next, ExitDecision::Retry);
    }
    match (&state.previous, previous_usable) {
        (Some(prev), true) => {
            next.bad.push(version.clone());
            next.active = prev.clone();
            next.previous = None;
            next.pending = false;
            next.crashes = 0;
            (next, ExitDecision::FallBack { to: prev.clone() })
        }
        _ => (
            next,
            ExitDecision::GiveUp {
                reason: format!("wersja {version} nie startuje, brak poprzedniej"),
            },
        ),
    }
}

/// Wersje do usunięcia przy sprzątaniu: zostaje `keep` najnowszych, zawsze aktywna, poprzednia
/// i wersje nowsze od aktywnej (przygotowana aktualizacja); wycofane (`bad`) idą pierwsze.
pub fn prune_victims(
    installed: &[Version],
    state: Option<&CurrentState>,
    keep: usize,
) -> Vec<Version> {
    let mut kept: BTreeSet<Version> = BTreeSet::new();
    if let Some(s) = state {
        kept.insert(s.active.clone());
        kept.extend(s.previous.iter().cloned());
        kept.extend(installed.iter().filter(|v| **v > s.active).cloned());
    }
    let mut sorted: Vec<&Version> = installed.iter().collect();
    sorted.sort_by(|a, b| b.cmp(a));
    for v in sorted {
        if kept.len() >= keep.max(1) {
            break;
        }
        if !state.is_some_and(|s| s.is_bad(v)) {
            kept.insert(v.clone());
        }
    }
    installed
        .iter()
        .filter(|v| !kept.contains(v))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
