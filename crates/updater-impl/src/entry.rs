//! Punkt wejścia stałego launchera `alfa.exe` i jego tryby (rozpoznawane **tylko** jako
//! pierwszy argument; wszystko inne trafia do aplikacji bez zmian):
//! - `alfa.exe [argumenty…]` — zwykły start (URI `alfa://`, ścieżki „Otwórz w Alfie”);
//! - `alfa.exe --alfa-restart [argumenty…]` — restart po aktualizacji z aplikacji: czeka (≤ 30 s)
//!   na zwolnienie blokady starej instancji, potem zwykły start (nowa aktywna wersja);
//! - `alfa.exe --alfa-installed <wersja>` — wywołuje instalator NSIS po skopiowaniu wersji do
//!   `versions\<ver>\`: `version.json`, przełączenie, sprzątanie (bez startu aplikacji);
//! - `alfa.exe --alfa-launcher-check` — samotest przed zamianą launchera (kod 73).
//!
//! Przy każdym starcie (poza samotestem) launcher najpierw zamienia przygotowany
//! `alfa.exe.new` ([`crate::selfupdate`]).

use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;

use semver::Version;
use updater_contract::{Updater, UpdaterError};

use crate::instance::wait_released;
use crate::launcher::{
    LaunchClock, Spawner, StdSpawner, SystemClock, locate_root, log_error, log_line, run,
};
use crate::selfupdate::{self, CHECK_ARG, CHECK_CODE, Swap};
use crate::{FsUpdater, UpdaterConfig};

/// Restart po aktualizacji (z aplikacji).
pub const RESTART_ARG: &str = "--alfa-restart";
/// Wersja skopiowana przez instalator.
pub const INSTALLED_ARG: &str = "--alfa-installed";
/// Ile czekać na zakończenie starej instancji przy restarcie.
pub const RESTART_WAIT_MS: u64 = 30_000;

/// Tryb pracy launchera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Zwykły start z argumentami dla aplikacji.
    Launch(Vec<OsString>),
    /// Restart: poczekaj na starą instancję, potem start.
    Restart(Vec<OsString>),
    /// Przyjęcie wersji z instalatora.
    Installed(String),
    /// Samotest.
    Check,
}

/// Tryb z argumentów (bez nazwy programu). Tryby specjalne tylko jako pierwszy argument.
pub fn parse_mode(args: Vec<OsString>) -> Mode {
    let first = args.first().and_then(|a| a.to_str()).map(str::to_owned);
    match first.as_deref() {
        Some(CHECK_ARG) if args.len() == 1 => Mode::Check,
        Some(RESTART_ARG) => Mode::Restart(args.into_iter().skip(1).collect()),
        Some(INSTALLED_ARG) if args.len() == 2 => Mode::Installed(
            args.get(1)
                .and_then(|a| a.to_str())
                .unwrap_or_default()
                .to_owned(),
        ),
        _ => Mode::Launch(args),
    }
}

/// Wykonuje tryb w katalogu instalacji `root` (`exe` — ścieżka bieżącego launchera).
pub fn execute(
    root: &Path,
    exe: Option<&Path>,
    mode: Mode,
    spawner: &dyn Spawner,
    clock: &dyn LaunchClock,
) -> Result<(), UpdaterError> {
    let updater = FsUpdater::new(UpdaterConfig::new(root))?;
    let layout = updater.layout().clone();
    match &mode {
        Mode::Check => return Ok(()),
        // Launcher z instalatora jest nowszy od przygotowanego wcześniej — ten odrzucamy.
        Mode::Installed(_) => {
            let _ = std::fs::remove_file(selfupdate::new_path(&layout));
            let _ = std::fs::remove_file(selfupdate::old_path(&layout));
        }
        Mode::Launch(_) | Mode::Restart(_) => {
            match selfupdate::swap_launcher(&layout, exe, spawner, clock) {
                Swap::Replaced => log_line(root, "zamieniono launcher na wersję z aktualizacji"),
                Swap::Rejected(why) => log_line(root, &format!("odrzucono nowy launcher: {why}")),
                Swap::Nothing => {}
            }
        }
    }
    match mode {
        Mode::Check => Ok(()),
        Mode::Installed(raw) => {
            let version = Version::parse(&raw)
                .map_err(|e| UpdaterError::invalid(format!("wersja „{raw}”: {e}")))?;
            updater.adopt_installed(&version).map(|_| ())
        }
        Mode::Restart(args) => {
            if !wait_released(root, clock, RESTART_WAIT_MS) {
                log_line(
                    root,
                    "restart: poprzednia instancja nie zakończyła się w 30 s",
                );
            }
            run(&updater, &args, spawner, clock, &updater.config().crash).map(|_| ())
        }
        Mode::Launch(args) => {
            run(&updater, &args, spawner, clock, &updater.config().crash).map(|_| ())
        }
    }
}

/// Punkt wejścia binarium `alfa` (`src/bin/alfa.rs`).
pub fn main_entry() -> ExitCode {
    let mode = parse_mode(std::env::args_os().skip(1).collect());
    if mode == Mode::Check {
        return ExitCode::from(CHECK_CODE);
    }
    let exe = std::env::current_exe().ok();
    let root = locate_root(exe.clone(), std::env::var_os("LOCALAPPDATA"));
    match execute(
        &root,
        exe.as_deref(),
        mode,
        &StdSpawner,
        &SystemClock::default(),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log_error(&root, &e);
            ExitCode::from(2)
        }
    }
}
