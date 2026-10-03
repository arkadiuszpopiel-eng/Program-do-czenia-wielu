//! Bezpieczna aktualizacja samego launchera `alfa.exe` („zapis obok + zamiana przy następnym
//! starcie”). Paczka wersji może zawierać nowy `alfa.exe`; po `mark_good` tej wersji jest on
//! kopiowany obok jako `alfa.exe.new` (+ `alfa.exe.new.sha256`, zapis atomowy). Przy następnym
//! starcie launcher: sprawdza skrót, uruchamia `alfa.exe.new --alfa-launcher-check` (samotest —
//! zły launcher nigdy nie zastępuje działającego), zamienia przez `rename` (działający plik
//! wykonywalny Windows można przemianować, nie nadpisać) i usuwa `alfa.exe.old` przy kolejnym
//! starcie. Bieżący proces kończy pracę starym kodem; nowy działa od następnego uruchomienia.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use updater_contract::{LAUNCHER_EXE, Layout, UpdaterError};

use crate::launcher::{LaunchClock, Spawner};
use crate::store::write_atomic;

/// Argument samotestu launchera.
pub const CHECK_ARG: &str = "--alfa-launcher-check";
/// Kod wyjścia poprawnego samotestu (nietypowy — przypadkowy program go nie zwróci).
pub const CHECK_CODE: u8 = 73;
/// Limit czasu samotestu.
const CHECK_TIMEOUT_MS: u64 = 10_000;

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(suffix);
    PathBuf::from(p)
}

/// `alfa.exe.new`.
pub fn new_path(layout: &Layout) -> PathBuf {
    suffixed(&layout.launcher, ".new")
}

/// `alfa.exe.old`.
pub fn old_path(layout: &Layout) -> PathBuf {
    suffixed(&layout.launcher, ".old")
}

fn sha_path(layout: &Layout) -> PathBuf {
    suffixed(&layout.launcher, ".new.sha256")
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Po `mark_good` wersji: jeśli jej paczka zawiera inny `alfa.exe` niż zainstalowany —
/// przygotowuje `alfa.exe.new` do zamiany przy następnym starcie. Zwraca, czy przygotowano.
pub fn stage_launcher(layout: &Layout, version: &semver::Version) -> Result<bool, UpdaterError> {
    let source = layout.version_dir(version).join(LAUNCHER_EXE);
    if !source.is_file() {
        return Ok(false);
    }
    let digest = sha256_file(&source)?;
    if layout.launcher.is_file() && sha256_file(&layout.launcher)? == digest {
        let _ = std::fs::remove_file(new_path(layout));
        let _ = std::fs::remove_file(sha_path(layout));
        return Ok(false);
    }
    write_atomic(&sha_path(layout), digest.as_bytes())?;
    let bytes = std::fs::read(&source)?;
    write_atomic(&new_path(layout), &bytes)?;
    Ok(true)
}

/// Wynik próby zamiany przy starcie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Swap {
    /// Nic do zamiany.
    Nothing,
    /// Zamieniono.
    Replaced,
    /// Odrzucono (zły skrót, nieudany samotest) — plik `.new` usunięty.
    Rejected(String),
}

fn reject(layout: &Layout, why: String) -> Swap {
    let _ = std::fs::remove_file(new_path(layout));
    let _ = std::fs::remove_file(sha_path(layout));
    Swap::Rejected(why)
}

fn self_check(exe: &Path, spawner: &dyn Spawner, clock: &dyn LaunchClock) -> Result<(), String> {
    let mut child = spawner
        .spawn(exe, &[OsString::from(CHECK_ARG)])
        .map_err(|e| format!("samotest nie wystartował: {e}"))?;
    let start = clock.now_ms();
    loop {
        match child.try_wait() {
            Ok(Some(code)) if code == i32::from(CHECK_CODE) => return Ok(()),
            Ok(Some(code)) => return Err(format!("samotest zwrócił {code}")),
            Err(e) => return Err(format!("samotest: {e}")),
            Ok(None) if clock.now_ms().saturating_sub(start) >= CHECK_TIMEOUT_MS => {
                let _ = child.kill();
                return Err("samotest przekroczył czas".to_owned());
            }
            Ok(None) => clock.sleep_ms(50),
        }
    }
}

/// Przy starcie launchera uruchomionego z `layout.launcher`: sprząta `alfa.exe.old`, a gotowy
/// `alfa.exe.new` (zgodny skrót + samotest) zamienia z bieżącym.
pub fn swap_launcher(
    layout: &Layout,
    current_exe: Option<&Path>,
    spawner: &dyn Spawner,
    clock: &dyn LaunchClock,
) -> Swap {
    let _ = std::fs::remove_file(old_path(layout));
    let new = new_path(layout);
    if !new.is_file() || current_exe != Some(layout.launcher.as_path()) {
        return Swap::Nothing;
    }
    let expected = std::fs::read_to_string(sha_path(layout)).unwrap_or_default();
    match sha256_file(&new) {
        Ok(actual) if actual == expected.trim() => {}
        _ => return reject(layout, "skrót alfa.exe.new niezgodny".to_owned()),
    }
    if let Err(why) = self_check(&new, spawner, clock) {
        return reject(layout, why);
    }
    if let Err(e) = std::fs::rename(&layout.launcher, old_path(layout)) {
        return Swap::Rejected(format!("nie przemianowano alfa.exe: {e}"));
    }
    if let Err(e) = std::fs::rename(&new, &layout.launcher) {
        let _ = std::fs::rename(old_path(layout), &layout.launcher);
        return Swap::Rejected(format!("nie zamieniono alfa.exe: {e}"));
    }
    let _ = std::fs::remove_file(sha_path(layout));
    Swap::Replaced
}
