//! Logika launchera `alfa.exe` (niezależna od Windows — testowana na katalogu tymczasowym):
//! wybór wersji, uruchomienie `versions\<ver>\alfa-desktop.exe` z **niezmienionymi**
//! argumentami (URI `alfa://`, ścieżki „Otwórz w Alfie”/„Wyślij do”), obserwacja przez okno
//! crash-loop, ponowienie albo powrót do poprzedniej wersji. Nowa wersja (przed `mark_good`)
//! jest obserwowana dłużej: wyjście z błędem przed potwierdzeniem albo brak `mark_good`
//! w `confirm_ms` (zawieszenie) = awaria → launcher ją zamyka i uruchamia poprzednią.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use semver::Version;
use updater_contract::{
    AppExit, CrashPolicy, ExitDecision, Layout, ROOT_DIR, Updater, UpdaterError, VERSIONS_DIR,
};

/// Uruchomiony proces aplikacji.
pub trait RunningApp {
    /// Kod wyjścia, jeśli proces się zakończył (zabity sygnałem → `-1`).
    fn try_wait(&mut self) -> std::io::Result<Option<i32>>;
    /// Zamyka proces (zawieszona nowa wersja bez `mark_good`, samotest po czasie).
    fn kill(&mut self) -> std::io::Result<()>;
}

/// Uruchamianie procesu (w testach — atrapa).
pub trait Spawner {
    /// Uruchamia `exe` z argumentami (bez zmian, bez powłoki).
    fn spawn(&self, exe: &Path, args: &[OsString]) -> std::io::Result<Box<dyn RunningApp>>;
}

/// Zegar launchera (w testach — wirtualny).
pub trait LaunchClock {
    /// Milisekundy od dowolnego punktu odniesienia (monotoniczne).
    fn now_ms(&self) -> u64;
    /// Uśpienie.
    fn sleep_ms(&self, ms: u64);
}

/// Zegar systemowy.
pub struct SystemClock(Instant);

impl Default for SystemClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}

impl LaunchClock for SystemClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

struct StdChild(std::process::Child);

impl RunningApp for StdChild {
    fn try_wait(&mut self) -> std::io::Result<Option<i32>> {
        Ok(self.0.try_wait()?.map(|s| s.code().unwrap_or(-1)))
    }
    fn kill(&mut self) -> std::io::Result<()> {
        self.0.kill()?;
        self.0.wait().map(|_| ())
    }
}

/// Uruchamianie przez `std::process::Command` (argumenty jako `OsString`, bez powłoki).
pub struct StdSpawner;

impl Spawner for StdSpawner {
    fn spawn(&self, exe: &Path, args: &[OsString]) -> std::io::Result<Box<dyn RunningApp>> {
        let child = std::process::Command::new(exe).args(args).spawn()?;
        Ok(Box::new(StdChild(child)))
    }
}

/// Wynik pracy launchera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchReport {
    /// Uruchomiona wersja.
    pub version: Version,
    /// Liczba prób.
    pub attempts: u32,
    /// Czy użyto wersji zapasowej (crash-loop albo uszkodzona aktywna).
    pub fell_back: bool,
}

const POLL_MS: u64 = 50;
/// Co ile ms po oknie crash-loop sprawdzać `mark_good` nowej wersji.
const CONFIRM_POLL_MS: u64 = 1_000;

/// Czy `version` czeka na potwierdzenie zdrowego startu (`mark_good`).
fn awaiting_confirmation(updater: &dyn Updater, version: &Version) -> bool {
    matches!(updater.state(), Ok(Some(s)) if s.active == *version && s.pending)
}

fn watch(
    updater: &dyn Updater,
    version: &Version,
    child: &mut dyn RunningApp,
    clock: &dyn LaunchClock,
    policy: &CrashPolicy,
) -> AppExit {
    let start = clock.now_ms();
    let mut pending = awaiting_confirmation(updater, version);
    loop {
        let after_ms = clock.now_ms().saturating_sub(start);
        match child.try_wait() {
            Ok(Some(code)) if code != 0 && pending && awaiting_confirmation(updater, version) => {
                return AppExit::Unconfirmed {
                    code: Some(code),
                    after_ms,
                };
            }
            Ok(Some(code)) => return AppExit::Exited { code, after_ms },
            // Stanu procesu nie da się odczytać — nie karzemy wersji.
            Err(_) => return AppExit::Running,
            Ok(None) => {}
        }
        if after_ms < policy.window_ms {
            clock.sleep_ms(POLL_MS);
            continue;
        }
        pending = pending && awaiting_confirmation(updater, version);
        if !pending {
            return AppExit::Running;
        }
        if after_ms >= policy.confirm_ms.max(policy.window_ms) {
            // Zawieszona nowa wersja: zamykamy ją i wracamy do poprzedniej.
            let _ = child.kill();
            return AppExit::Unconfirmed {
                code: None,
                after_ms,
            };
        }
        clock.sleep_ms(CONFIRM_POLL_MS);
    }
}

/// Uruchamia aplikację z polityką crash-loop: zdrowy start kończy pracę launchera (proces
/// aplikacji działa dalej samodzielnie).
pub fn run(
    updater: &dyn Updater,
    args: &[OsString],
    spawner: &dyn Spawner,
    clock: &dyn LaunchClock,
    policy: &CrashPolicy,
) -> Result<LaunchReport, UpdaterError> {
    let max_attempts = policy
        .max_quick_crashes
        .max(1)
        .saturating_mul(2)
        .saturating_add(2);
    let mut fell_back = false;
    for attempt in 1..=max_attempts {
        let choice = updater.select_launch()?;
        fell_back |= choice.fallback;
        let exit = match spawner.spawn(&choice.exe, args) {
            Ok(mut child) => watch(updater, &choice.version, child.as_mut(), clock, policy),
            Err(e) => AppExit::FailedToStart {
                reason: e.to_string(),
            },
        };
        match updater.record_exit(&choice.version, &exit)? {
            ExitDecision::Healthy => {
                return Ok(LaunchReport {
                    version: choice.version,
                    attempts: attempt,
                    fell_back,
                });
            }
            ExitDecision::Retry => {}
            ExitDecision::FallBack { .. } => fell_back = true,
            ExitDecision::GiveUp { reason } => {
                return Err(UpdaterError::NoUsableVersion { reason });
            }
        }
    }
    Err(UpdaterError::NoUsableVersion {
        reason: format!("aplikacja nie wystartowała po {max_attempts} próbach"),
    })
}

/// Katalog instalacji: katalog `alfa.exe`, jeśli zawiera `versions\`; inaczej `%LOCALAPPDATA%\Alfa`.
pub fn locate_root(exe: Option<PathBuf>, local_app_data: Option<OsString>) -> PathBuf {
    if let Some(dir) = exe.as_deref().and_then(Path::parent)
        && dir.join(VERSIONS_DIR).is_dir()
    {
        return dir.to_path_buf();
    }
    match local_app_data {
        Some(lad) => Layout::for_local_app_data(Path::new(&lad)).root,
        None => exe
            .as_deref()
            .and_then(Path::parent)
            .map_or_else(|| PathBuf::from(ROOT_DIR), Path::to_path_buf),
    }
}

/// Katalog instalacji widziany z aplikacji: `<root>` dla `<root>\versions\<ver>\alfa-desktop.exe`
/// (instalacja przez launcher); inaczej `fallback` (build deweloperski, testy).
pub fn app_install_root(exe: Option<&Path>, fallback: &Path) -> PathBuf {
    let root = exe
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|versions| versions.file_name().is_some_and(|n| n == VERSIONS_DIR))
        .and_then(Path::parent);
    root.map_or_else(|| fallback.to_path_buf(), Path::to_path_buf)
}

/// Maksymalny rozmiar `launcher.log` (potem plik zaczyna się od nowa).
const LOG_MAX: u64 = 64 * 1024;

/// Dopisuje błąd do `<root>\launcher.log` (launcher nie ma konsoli ani okna).
pub fn log_error(root: &Path, error: &UpdaterError) {
    log_line(root, &error.to_string());
}

/// Dopisuje wiersz do `<root>\launcher.log` (≤ 64 KiB, potem od nowa).
pub fn log_line(root: &Path, message: &str) {
    let path = root.join("launcher.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > LOG_MAX) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{} {message}", chrono::Utc::now().to_rfc3339());
    }
}
