//! Dziennik diagnostyczny procesów Alfy (PLAN §13; SPEC `core-log`, sekcja „Fala 5”).
//!
//! Subskrybent `tracing` instalowany w `main` każdego procesu (powłoka Tauri `alfa-desktop`,
//! `alfa-broker`, `alfa-broker-ui`, `alfa-watchdog`). Bez niego wszystkie `tracing::…` w
//! aplikacji przepadały. Zapis:
//! - plik `%LOCALAPPDATA%\Alfa\logs\<proces>.<RRRR-MM-DD>.<NNN>.log` (data i czas UTC), jedno
//!   zdarzenie = jedna linia tekstu; rotacja po dniu i po 10 MiB, najwyżej 14 plików na proces,
//!   retencja 7 dni (`[logs] file_days`, 1–90) — [`Rotation`];
//! - w buildzie debug także stderr (konsola `cargo tauri dev`);
//! - poziom: zmienna `ALFA_LOG` (pierwszeństwo), potem `[logs] level` z konfiguracji, domyślnie
//!   `info`; biblioteki spoza Alfy najwyżej `warn`, chyba że wskazane jawnie ([`Filter`]).
//!
//! **Bez sekretów i treści** ([`Redaction`]): pola o nazwach sekretów → `[REDACTED]`, pola
//! treści (tekst rozmowy, transkrypcja, obraz, schowek) → pominięte, pozostałe wartości przez
//! redaktor `core-log` (`RegexRedactor`) + wzorce dodatkowe, obcięte i bez znaków sterujących.
//! Pliki `.log` nie trafiają do eksportu `.alfa` (kategoria „Logi” bierze tylko `*.ndjson`).
//! Panika (przed `abort` w release) jest zapisywana do dziennika ([`install`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod file;
mod filter;
mod redact;
mod subscriber;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

pub use file::{
    Clock, DEFAULT_MAX_FILE_BYTES, DEFAULT_MAX_FILES, DEFAULT_RETENTION_DAYS, LogFile, Rotation,
    file_name, list as list_files, parse_name,
};
pub use filter::{DEFAULT_LEVEL, Filter, FilterError, is_alfa_target};
pub use redact::{
    HARD_CAP, MAX_FIELD_CHARS, MAX_FIELDS, MAX_MESSAGE_CHARS, Redaction, is_content_field,
    is_secret_field,
};
pub use subscriber::FileSubscriber;

use crate::file::RollingFile;
use crate::subscriber::{Shared, lock};

/// Zmienna środowiskowa poziomu (pierwszeństwo przed konfiguracją).
pub const ENV_VAR: &str = "ALFA_LOG";
/// Klucz konfiguracji poziomu (SPEC `core-log`).
pub const LEVEL_KEY: &str = "logs.level";
/// Klucz konfiguracji retencji pliku dziennika (dni).
pub const RETENTION_KEY: &str = "logs.file_days";
/// Zakres retencji z konfiguracji (dni).
pub const RETENTION_RANGE: std::ops::RangeInclusive<u32> = 1..=90;

/// Prefiksy plików procesów.
pub mod process {
    /// Powłoka Tauri (aplikacja).
    pub const DESKTOP: &str = "alfa";
    /// Usługa Brokera.
    pub const BROKER: &str = "alfa-broker";
    /// Okno zatwierdzeń Brokera.
    pub const BROKER_UI: &str = "alfa-broker-ui";
    /// Watchdog (kill-switch).
    pub const WATCHDOG: &str = "alfa-watchdog";
}

/// Ustawienia dziennika procesu.
#[derive(Clone)]
pub struct LogConfig {
    /// Prefiks plików ([`process`]).
    pub process: String,
    /// Katalog (`None` — bez pliku).
    pub dir: Option<PathBuf>,
    /// Specyfikacja poziomu ([`Filter::parse`]).
    pub filter: String,
    /// Czy `filter` pochodzi z `ALFA_LOG` (wtedy konfiguracja go nie zmienia).
    pub filter_from_env: bool,
    /// Kopia na stderr.
    pub stderr: bool,
    /// Limity pliku.
    pub rotation: Rotation,
    /// Zegar.
    pub clock: Clock,
    /// Zapis paniki do dziennika ([`install`]).
    pub panic_hook: bool,
}

impl LogConfig {
    /// Domyślne dla procesu: poziom z `ALFA_LOG` albo `info`, stderr w buildzie debug.
    pub fn for_process(process: &str, dir: Option<PathBuf>) -> Self {
        let env = env_filter();
        Self {
            process: process.to_owned(),
            dir,
            filter_from_env: env.is_some(),
            filter: env.unwrap_or_else(|| DEFAULT_LEVEL.to_owned()),
            stderr: cfg!(debug_assertions),
            rotation: Rotation::default(),
            clock: Arc::new(chrono::Utc::now),
            panic_hook: true,
        }
    }
}

/// Wartość `ALFA_LOG` (pusta = brak).
pub fn env_filter() -> Option<String> {
    std::env::var(ENV_VAR)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// Katalog logów jak `AppPaths::from_env().logs()`: `%LOCALAPPDATA%\Alfa\logs` (Windows),
/// `~/.local/share/alfa/logs` (tryb deweloperski poza Windows). Dla usługi Brokera —
/// `%LOCALAPPDATA%` konta usługi.
pub fn default_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
    if cfg!(windows) {
        return var("LOCALAPPDATA").map(|l| l.join("Alfa").join("logs"));
    }
    var("HOME").map(|h| h.join(".local").join("share").join("alfa").join("logs"))
}

/// Uchwyt dziennika: zmiana poziomu i retencji w czasie działania, stan zapisu.
#[derive(Clone)]
pub struct LogHandle(Arc<Shared>);

impl LogHandle {
    /// Zmienia poziom (przebudowuje pamięć podręczną wywołań `tracing`).
    pub fn set_filter(&self, spec: &str) -> Result<(), FilterError> {
        let filter = Filter::parse(spec)?;
        *self
            .0
            .filter
            .write()
            .unwrap_or_else(PoisonError::into_inner) = filter;
        tracing_core::callsite::rebuild_interest_cache();
        Ok(())
    }

    /// Bieżący filtr.
    pub fn filter(&self) -> Filter {
        self.0.filter()
    }

    /// Zmienia retencję (dni, przycięte do [`RETENTION_RANGE`]) i od razu usuwa stare pliki.
    pub fn set_retention_days(&self, days: u32) -> std::io::Result<usize> {
        let days = days.clamp(*RETENTION_RANGE.start(), *RETENTION_RANGE.end());
        match lock(&self.0.file).as_mut() {
            Some(file) => file.set_retention_days(days),
            None => Ok(0),
        }
    }

    /// Ustawienia z konfiguracji (`[logs] level`, `[logs] file_days`); poziom z `ALFA_LOG` ma
    /// pierwszeństwo. Zwraca problemy (błędna wartość zostawia poprzednią).
    pub fn apply_settings(&self, level: Option<&str>, retention_days: Option<u64>) -> Vec<String> {
        let mut problems = Vec::new();
        if let Some(level) = level.filter(|_| !self.0.filter_from_env)
            && let Err(e) = self.set_filter(level)
        {
            problems.push(format!("{LEVEL_KEY}: {e}"));
        }
        if let Some(days) = retention_days {
            let days = u32::try_from(days).unwrap_or(u32::MAX);
            if let Err(e) = self.set_retention_days(days) {
                problems.push(format!("{RETENTION_KEY}: {e}"));
            }
        }
        problems
    }

    /// Katalog dziennika (`None` — plik niedostępny).
    pub fn dir(&self) -> Option<PathBuf> {
        lock(&self.0.file).as_ref().map(|f| f.dir().to_path_buf())
    }

    /// Bieżący plik (po pierwszym zapisie).
    pub fn current_file(&self) -> Option<PathBuf> {
        lock(&self.0.file)
            .as_ref()
            .and_then(|f| f.current().map(PathBuf::from))
    }

    /// Ostatni problem (otwarcie pliku, błędny `ALFA_LOG`, nieudany zapis).
    pub fn problem(&self) -> Option<String> {
        lock(&self.0.problem).clone()
    }

    /// Liczba nieudanych zapisów do pliku.
    pub fn failed_writes(&self) -> u64 {
        self.0.failed_writes.load(Ordering::Relaxed)
    }
}

/// Subskrybent i uchwyt bez instalacji globalnej (testy: `tracing::subscriber::with_default`).
pub fn build(config: LogConfig) -> (FileSubscriber, LogHandle) {
    let mut problems = Vec::new();
    let filter = Filter::parse(&config.filter).unwrap_or_else(|e| {
        problems.push(format!(
            "{ENV_VAR}/{LEVEL_KEY}: {e} — używam „{DEFAULT_LEVEL}”"
        ));
        Filter::default()
    });
    let file = config.dir.and_then(|dir| {
        let shown = dir.display().to_string();
        RollingFile::open(dir, &config.process, config.rotation, config.clock.clone())
            .map_err(|e| problems.push(format!("katalog dziennika {shown}: {e}")))
            .ok()
    });
    let problem = (!problems.is_empty()).then(|| problems.join("; "));
    let shared = Arc::new(Shared {
        filter: std::sync::RwLock::new(filter),
        file: Mutex::new(file),
        stderr: config.stderr,
        redaction: Redaction::default(),
        clock: config.clock,
        next_span: AtomicU64::new(1),
        problem: Mutex::new(problem),
        failed_writes: AtomicU64::new(0),
        filter_from_env: config.filter_from_env,
    });
    (FileSubscriber(shared.clone()), LogHandle(shared))
}

/// Subskrybent jest już zainstalowany w tym procesie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlreadyInstalled;

impl std::fmt::Display for AlreadyInstalled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("subskrybent dziennika jest już zainstalowany")
    }
}

impl std::error::Error for AlreadyInstalled {}

/// Instaluje subskrybenta globalnie (raz na proces) i — gdy `panic_hook` — zapis paniki.
/// Problem przy starcie (np. katalog niedostępny) jest zgłaszany zdarzeniem `warn`.
pub fn install(config: LogConfig) -> Result<LogHandle, AlreadyInstalled> {
    let panic_hook = config.panic_hook;
    let (subscriber, handle) = build(config);
    tracing::subscriber::set_global_default(subscriber).map_err(|_| AlreadyInstalled)?;
    if panic_hook {
        install_panic_hook();
    }
    if let Some(problem) = handle.problem() {
        tracing::warn!(problem = %problem, "dziennik diagnostyczny: problem przy starcie");
    }
    Ok(handle)
}

/// Panika → zdarzenie `error` (miejsce i komunikat, z redakcją), potem poprzedni hook.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let payload = info.payload_as_str().unwrap_or("(bez komunikatu)");
        tracing::error!(target: "alfa_panic", miejsce = %location, "panika: {payload}");
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uchwyt trafia do domknięcia `setup` Tauri (`Send + 'static`) i do wątków.
    #[test]
    fn handle_and_subscriber_cross_threads() {
        fn send_sync<T: Send + Sync + 'static>() {}
        send_sync::<LogHandle>();
        send_sync::<FileSubscriber>();
        send_sync::<LogConfig>();
    }

    #[test]
    fn default_dir_matches_app_paths_layout() {
        let app = if cfg!(windows) { "Alfa" } else { "alfa" };
        if let Some(dir) = default_dir() {
            assert!(
                dir.ends_with(std::path::Path::new(app).join("logs")),
                "{dir:?}"
            );
        }
        let config = LogConfig::for_process(process::WATCHDOG, None);
        assert_eq!(config.process, "alfa-watchdog");
        assert_eq!(config.stderr, cfg!(debug_assertions));
        assert_eq!(config.rotation, Rotation::default());
    }
}
