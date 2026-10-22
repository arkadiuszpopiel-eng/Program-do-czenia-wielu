//! Dziennik rozruchu: linie dziennika (już po redakcji sekretów) od startu procesu do zamknięcia okna
//! rozruchu ([`crate::LogHandle::boot_finish`]) — do podglądu w programie i sygnału „rozruch OK /
//! z uwagami / z problemem”. Kopia aplikacji bez okna konsoli nie pokazuje dziennika nigdzie indziej.

use std::sync::Mutex;
use std::time::Instant;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use tracing::Level;

use crate::subscriber::lock;

/// Najwięcej linii w dzienniku rozruchu (dalsze tylko liczone).
pub const MAX_BOOT_LINES: usize = 500;

/// Linia dziennika rozruchu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BootLine {
    /// `ERROR`, `WARN`, `INFO`, `DEBUG` albo `TRACE`.
    pub level: &'static str,
    /// Linia jak w pliku dziennika (czas, poziom, źródło, treść), bez końca wiersza.
    pub text: String,
}

/// Werdykt rozruchu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootVerdict {
    /// Bez ostrzeżeń i błędów.
    Ok,
    /// Ostrzeżenia, bez błędów.
    Warnings,
    /// Co najmniej jeden błąd.
    Errors,
}

/// Raport rozruchu dla UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BootReport {
    /// Start procesu (RFC 3339, UTC).
    pub started_at: String,
    /// Okno rozruchu zamknięte (raport już się nie zmieni).
    pub finished: bool,
    /// Czas rozruchu (ms): do zamknięcia okna, a przed nim — dotychczasowy.
    pub elapsed_ms: u64,
    /// Werdykt.
    pub verdict: BootVerdict,
    /// Liczba ostrzeżeń.
    pub warnings: u32,
    /// Liczba błędów.
    pub errors: u32,
    /// Linie (najwyżej [`MAX_BOOT_LINES`]).
    pub lines: Vec<BootLine>,
    /// Linie pominięte ponad limit.
    pub dropped: u32,
    /// Plik dziennika (pełny zapis).
    pub log_file: Option<String>,
}

#[derive(Default)]
struct State {
    finished_ms: Option<u64>,
    warnings: u32,
    errors: u32,
    lines: Vec<BootLine>,
    dropped: u32,
}

/// Zbieranie linii rozruchu (stan subskrybenta).
pub(crate) struct BootJournal {
    started: Instant,
    started_at: DateTime<Utc>,
    state: Mutex<State>,
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::ERROR => "ERROR",
        Level::WARN => "WARN",
        Level::INFO => "INFO",
        Level::DEBUG => "DEBUG",
        Level::TRACE => "TRACE",
    }
}

impl BootJournal {
    pub fn new(started_at: DateTime<Utc>) -> Self {
        Self {
            started: Instant::now(),
            started_at,
            state: Mutex::new(State::default()),
        }
    }

    /// Linia zapisana do dziennika (pomijana po zamknięciu okna rozruchu).
    pub fn record(&self, level: Level, line: &str) {
        let mut s = lock(&self.state);
        if s.finished_ms.is_some() {
            return;
        }
        match level {
            Level::ERROR => s.errors = s.errors.saturating_add(1),
            Level::WARN => s.warnings = s.warnings.saturating_add(1),
            _ => {}
        }
        if s.lines.len() < MAX_BOOT_LINES {
            let text = line.trim_end_matches(['\r', '\n']).to_owned();
            s.lines.push(BootLine {
                level: level_name(level),
                text,
            });
        } else {
            s.dropped = s.dropped.saturating_add(1);
        }
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Zamyka okno rozruchu (kolejne wywołania nic nie zmieniają).
    pub fn finish(&self) {
        let ms = self.elapsed_ms();
        let mut s = lock(&self.state);
        if s.finished_ms.is_none() {
            s.finished_ms = Some(ms);
        }
    }

    pub fn report(&self, log_file: Option<String>) -> BootReport {
        let now = self.elapsed_ms();
        let s = lock(&self.state);
        let verdict = if s.errors > 0 {
            BootVerdict::Errors
        } else if s.warnings > 0 {
            BootVerdict::Warnings
        } else {
            BootVerdict::Ok
        };
        BootReport {
            started_at: self.started_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            finished: s.finished_ms.is_some(),
            elapsed_ms: s.finished_ms.unwrap_or(now),
            verdict,
            warnings: s.warnings,
            errors: s.errors,
            lines: s.lines.clone(),
            dropped: s.dropped,
            log_file,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_counts_and_window() {
        let j = BootJournal::new(Utc::now());
        j.record(Level::INFO, "a\n");
        assert_eq!(j.report(None).verdict, BootVerdict::Ok);
        j.record(Level::WARN, "b\r\n");
        assert_eq!(j.report(None).verdict, BootVerdict::Warnings);
        j.record(Level::ERROR, "c");
        let r = j.report(Some("plik.log".into()));
        assert_eq!(
            (r.verdict, r.warnings, r.errors),
            (BootVerdict::Errors, 1, 1)
        );
        assert_eq!(
            r.lines[1],
            BootLine {
                level: "WARN",
                text: "b".into()
            }
        );
        assert!(!r.finished);
        j.finish();
        j.record(Level::ERROR, "po rozruchu");
        let r = j.report(None);
        assert!(r.finished && r.errors == 1 && r.lines.len() == 3);
    }

    #[test]
    fn lines_are_capped_but_counted() {
        let j = BootJournal::new(Utc::now());
        for _ in 0..MAX_BOOT_LINES + 3 {
            j.record(Level::WARN, "x");
        }
        let r = j.report(None);
        assert_eq!((r.lines.len(), r.dropped), (MAX_BOOT_LINES, 3));
        assert_eq!(r.warnings, u32::try_from(MAX_BOOT_LINES + 3).unwrap_or(0));
    }
}
