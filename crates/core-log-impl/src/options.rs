//! Ustawienia log-writera (`[logs]` w konfiguracji) i zegar.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use core_log_contract::LogStream;

/// Źródło czasu zapisu (`written_at`, retencja). W testach — zegar wirtualny.
pub trait Clock: Send + Sync {
    /// Bieżący czas.
    fn now(&self) -> DateTime<Utc>;
}

/// Zegar systemowy.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

impl<F: Fn() -> DateTime<Utc> + Send + Sync> Clock for F {
    fn now(&self) -> DateTime<Utc> {
        self()
    }
}

/// Limity jednego strumienia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamLimits {
    /// Limit dysku strumienia w bajtach; po przekroczeniu usuwane są najstarsze segmenty.
    pub disk_limit_bytes: u64,
    /// Retencja w dniach (segmenty starsze są usuwane); `None` = tylko limit dysku.
    pub retention_days: Option<u32>,
}

/// Domyślny limit dysku strumienia: 512 MiB (SPEC: 2 GiB łącznie na 4 strumienie).
pub const DEFAULT_STREAM_DISK_LIMIT: u64 = 512 * 1024 * 1024;
/// Domyślny rozmiar segmentu (rotacja): 8 MiB.
pub const DEFAULT_SEGMENT_BYTES: u64 = 8 * 1024 * 1024;
/// Domyślna retencja Narzędzi/GUI (PLAN §13): 7 dni.
pub const DEFAULT_TOOLS_GUI_RETENTION_DAYS: u32 = 7;

/// Ustawienia `FileLogSink`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogOptions {
    /// Katalog logów (np. `%LOCALAPPDATA%\Alfa\logs`); każdy strumień ma podkatalog.
    pub root: PathBuf,
    /// Rozmiar segmentu, po którym następuje rotacja.
    pub max_segment_bytes: u64,
    /// Limity per strumień (brak wpisu = `default_limits`).
    pub limits: BTreeMap<LogStream, StreamLimits>,
    /// Limity dla strumieni bez wpisu.
    pub default_limits: StreamLimits,
}

impl LogOptions {
    /// Ustawienia domyślne dla katalogu (Narzędzia/GUI: retencja 7 dni).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let default_limits = StreamLimits {
            disk_limit_bytes: DEFAULT_STREAM_DISK_LIMIT,
            retention_days: None,
        };
        let tools_gui = StreamLimits {
            retention_days: Some(DEFAULT_TOOLS_GUI_RETENTION_DAYS),
            ..default_limits
        };
        Self {
            root: root.into(),
            max_segment_bytes: DEFAULT_SEGMENT_BYTES,
            limits: [(LogStream::ToolsGui, tools_gui)].into(),
            default_limits,
        }
    }

    /// Limity strumienia.
    pub fn limits(&self, stream: LogStream) -> StreamLimits {
        self.limits
            .get(&stream)
            .copied()
            .unwrap_or(self.default_limits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_spec() {
        let o = LogOptions::new("/tmp/x");
        assert_eq!(o.limits(LogStream::ToolsGui).retention_days, Some(7));
        assert_eq!(o.limits(LogStream::Voice).retention_days, None);
        assert_eq!(
            o.limits(LogStream::ModelCalls).disk_limit_bytes,
            DEFAULT_STREAM_DISK_LIMIT
        );
        let fixed = || DateTime::<Utc>::from_timestamp(9, 0).unwrap();
        assert_eq!(fixed.now().timestamp(), 9);
        assert!(SystemClock.now().timestamp() > 0);
    }
}
