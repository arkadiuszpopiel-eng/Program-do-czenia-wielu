//! Komunikaty `alfa-watchdog` dla aplikacji (jedna linia JSON na stdout, `app_safety::watchdog`):
//! `{"event":"ready"}` — skrót `Ctrl+Shift+F12` zarejestrowany przez watchdoga (aplikacja go nie
//! rejestruje drugi raz); `{"event":"kill_switch",…}` — kill-switch wykonany poza UI (drzewa,
//! Broker), aplikacja zatrzymuje generacje, przebiegi, mowę i drzewa narzędzi („STOP WSZYSTKIEGO”).
//! Kanał to anonimowy potok stdout procesu potomnego — tylko aplikacja ma jego koniec do odczytu.

use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::watch;

use crate::children::{ChildProc, LastLine, pump_lines};
use crate::status::KernelStatus;

/// Komunikat watchdoga.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Notice {
    /// Skrót zarejestrowany, watchdog działa.
    Ready,
    /// Kill-switch wykonany.
    KillSwitch {
        /// Czas od naciśnięcia do zakończenia (µs).
        #[serde(default)]
        latency_us: u64,
    },
}

/// Parsuje linię; inne linie (dziennik) — `None`.
pub fn parse(line: &str) -> Option<Notice> {
    serde_json::from_str(line.trim()).ok()
}

/// Podłącza wyjście watchdoga: gotowość i koniec → stan, kill-switch → licznik `kills`.
pub fn attach(
    child: &mut dyn ChildProc,
    status: &Arc<KernelStatus>,
    kills: &Arc<watch::Sender<u64>>,
) -> LastLine {
    let last = LastLine::default();
    if let Some(err) = child.take_stderr() {
        pump_lines("watchdog-log", err, LastLine::default(), |line| {
            tracing::info!(target: "alfa-watchdog", "{line}");
        });
    }
    let Some(out) = child.take_stdout() else {
        return last;
    };
    let (status_line, kills) = (status.clone(), kills.clone());
    let ended = EndGuard(status.clone());
    pump_lines("watchdog", out, last.clone(), move |line| {
        let _keep = &ended;
        match parse(line) {
            Some(Notice::Ready) => status_line.set_watchdog(true),
            Some(Notice::KillSwitch { latency_us }) => {
                tracing::warn!(latency_us, "kill-switch watchdoga (Ctrl+Shift+F12)");
                kills.send_modify(|n| *n = n.wrapping_add(1));
            }
            None => tracing::info!(target: "alfa-watchdog", "{line}"),
        }
    });
    last
}

/// Koniec strumienia (watchdog zakończony) → aplikacja przejmuje skrót.
struct EndGuard(Arc<KernelStatus>);

impl Drop for EndGuard {
    fn drop(&mut self) {
        self.0.set_watchdog(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_lines_only() {
        assert_eq!(parse(r#"{"event":"ready"}"#), Some(Notice::Ready));
        assert_eq!(
            parse(r#"{"event":"kill_switch","reason":{"source":"hotkey"},"latency_us":120}"#),
            Some(Notice::KillSwitch { latency_us: 120 })
        );
        assert_eq!(
            parse(r#"{"event":"kill_switch"}"#),
            Some(Notice::KillSwitch { latency_us: 0 })
        );
        assert_eq!(
            parse("[alfa-watchdog] kill-switch Ctrl+Shift+F12 aktywny"),
            None
        );
        assert_eq!(parse(r#"{"event":"other"}"#), None);
    }
}
