//! Pliki instalacji: `current.json` (odczyt odporny na uszkodzenie, zapis atomowy: plik
//! tymczasowy + `fsync` + `rename`) i katalogi wersji (nazwa = kanoniczny semver, plik
//! `alfa-desktop.exe`, opcjonalny `version.json` zgodny z nazwą).

use std::io::Write;
use std::path::Path;

use semver::Version;
use updater_contract::{CurrentState, Layout, STATE_SCHEMA, UpdaterError, VERSION_FILE};

/// Stan z `current.json`; brak pliku, zły JSON albo nieznany schemat → `None` (launcher wybierze
/// wtedy najnowszą poprawną wersję).
pub fn read_state(layout: &Layout) -> Option<CurrentState> {
    let bytes = std::fs::read(&layout.current).ok()?;
    serde_json::from_slice::<CurrentState>(&bytes)
        .ok()
        .filter(|s| s.schema == STATE_SCHEMA)
}

/// Atomowy zapis `current.json`.
pub fn write_state(layout: &Layout, state: &CurrentState) -> Result<(), UpdaterError> {
    std::fs::create_dir_all(&layout.root)?;
    let json = serde_json::to_vec_pretty(state).map_err(UpdaterError::invalid)?;
    let tmp = layout
        .root
        .join(format!(".{}.tmp", updater_contract::CURRENT_FILE));
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(&json)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, &layout.current).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

/// Wszystkie katalogi o kanonicznej nazwie semver (także uszkodzone), rosnąco.
pub fn version_dirs(layout: &Layout) -> Vec<Version> {
    let Ok(entries) = std::fs::read_dir(&layout.versions) else {
        return Vec::new();
    };
    let mut out: Vec<Version> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            Version::parse(&name).ok().filter(|v| v.to_string() == name)
        })
        .collect();
    out.sort();
    out
}

#[derive(serde::Deserialize)]
struct VersionFile {
    version: String,
}

/// Czy wersja nadaje się do uruchomienia: plik wykonywalny istnieje, `version.json` (jeśli jest)
/// zgadza się z nazwą katalogu.
pub fn is_usable(layout: &Layout, version: &Version) -> bool {
    let dir = layout.version_dir(version);
    if !layout.app_exe(version).is_file() {
        return false;
    }
    match std::fs::read(dir.join(VERSION_FILE)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => false,
        Ok(bytes) => serde_json::from_slice::<VersionFile>(&bytes)
            .is_ok_and(|f| f.version == version.to_string()),
    }
}

/// Usuwa katalog wersji (musi leżeć w `versions`).
pub fn remove_version(layout: &Layout, version: &Version) -> Result<(), UpdaterError> {
    let dir = layout.version_dir(version);
    if !dir.starts_with(&layout.versions) || dir == Path::new(&layout.versions) {
        return Err(UpdaterError::invalid("katalog wersji poza versions"));
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}
