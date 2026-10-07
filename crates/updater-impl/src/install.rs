//! Rozpakowanie zweryfikowanej paczki do `versions\<ver>\`: najpierw do katalogu roboczego
//! `versions\.staging-<ver>` (nie jest kanoniczną nazwą semver — launcher i sprzątanie go nie
//! widzą), potem `rename` na miejsce. Reguły jak w `transfer`: ścieżki bez `..`/`\`/`:`/nazw
//! urządzeń, bez dowiązań, bez duplikatów (także różniących się wielkością liter — NTFS),
//! limity liczby wpisów, rozmiaru wpisu i całości oraz stopnia kompresji; rzeczywista liczba
//! bajtów nie może przekroczyć zadeklarowanej (CRC sprawdza `zip`). Wymagane `alfa-desktop.exe`
//! i `version.json` zgodny z wersją z podpisanego manifestu.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use semver::Version;
use updater_contract::{
    APP_EXE, Layout, PackageLimits, UpdaterError, VERSION_FILE, validate_package_path,
};
use zip::ZipArchive;

const STAGING_PREFIX: &str = ".staging-";
/// Tryb pliku Unix: dowiązanie symboliczne.
const S_IFLNK: u32 = 0o120_000;
const S_IFMT: u32 = 0o170_000;

fn zip_err(e: zip::result::ZipError) -> UpdaterError {
    UpdaterError::unsafe_package(format!("uszkodzone archiwum: {e}"))
}

/// Usuwa pozostałości przerwanych rozpakowań (`versions\.staging-*`).
pub fn clean_staging_dirs(layout: &Layout) {
    let Ok(entries) = std::fs::read_dir(&layout.versions) else {
        return;
    };
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(STAGING_PREFIX)
        {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Rozpakowuje paczkę `package` wersji `version` do `versions\<ver>\` (atomowo przez `rename`).
/// Istniejący katalog tej wersji (np. uszkodzony po przerwanej instalacji) jest zastępowany —
/// wywołujący pilnuje, by nie była to wersja uruchomiona.
pub fn install_package(
    layout: &Layout,
    version: &Version,
    package: &Path,
    limits: &PackageLimits,
) -> Result<PathBuf, UpdaterError> {
    std::fs::create_dir_all(&layout.versions)?;
    let work = layout.versions.join(format!("{STAGING_PREFIX}{version}"));
    if work.exists() {
        std::fs::remove_dir_all(&work)?;
    }
    std::fs::create_dir_all(&work)?;
    let result = extract(package, &work, limits).and_then(|()| check_layout(&work, version));
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&work);
        return Err(e);
    }
    let target = layout.version_dir(version);
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::rename(&work, &target).inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&work);
    })?;
    Ok(target)
}

fn extract(package: &Path, dest: &Path, limits: &PackageLimits) -> Result<(), UpdaterError> {
    let mut archive = ZipArchive::new(BufReader::new(File::open(package)?)).map_err(zip_err)?;
    let entries = archive.len() as u64;
    if entries > limits.max_entries {
        return Err(UpdaterError::unsafe_package(format!(
            "{entries} wpisów > {}",
            limits.max_entries
        )));
    }
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(zip_err)?;
        let name = entry.name().to_owned();
        validate_package_path(&name)?;
        if !seen.insert(name.trim_end_matches('/').to_lowercase()) {
            return Err(UpdaterError::unsafe_package(format!(
                "powtórzony wpis „{name}”"
            )));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & S_IFMT == S_IFLNK)
        {
            return Err(UpdaterError::unsafe_package(format!("dowiązanie „{name}”")));
        }
        let path = dest.join(&name);
        if entry.is_dir() {
            std::fs::create_dir_all(&path)?;
            continue;
        }
        let size = entry.size();
        total = limits.admit(size, entry.compressed_size(), total)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = File::create_new(&path)?;
        let copied = std::io::copy(&mut (&mut entry).take(size.saturating_add(1)), &mut out)?;
        if copied != size {
            return Err(UpdaterError::unsafe_package(format!(
                "wpis „{name}”: {copied} B zamiast zadeklarowanych {size} B"
            )));
        }
        out.flush()?;
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct VersionFile {
    version: String,
}

fn check_layout(dir: &Path, version: &Version) -> Result<(), UpdaterError> {
    if !dir.join(APP_EXE).is_file() {
        return Err(UpdaterError::unsafe_package(format!("brak {APP_EXE}")));
    }
    let raw = std::fs::read(dir.join(VERSION_FILE))
        .map_err(|_| UpdaterError::unsafe_package(format!("brak {VERSION_FILE}")))?;
    let file: VersionFile = serde_json::from_slice(&raw)
        .map_err(|e| UpdaterError::unsafe_package(format!("{VERSION_FILE}: {e}")))?;
    if file.version != version.to_string() {
        return Err(UpdaterError::unsafe_package(format!(
            "{VERSION_FILE} ma wersję {} zamiast {version}",
            file.version
        )));
    }
    Ok(())
}
