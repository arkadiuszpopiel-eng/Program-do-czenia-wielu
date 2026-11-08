//! Pliki instalacji: `current.json` (odczyt odporny na uszkodzenie, zapis atomowy: plik
//! tymczasowy + `fsync` + `rename`) i katalogi wersji (nazwa = kanoniczny semver, plik
//! `alfa-desktop.exe`, opcjonalny `version.json` zgodny z nazwą).

use std::io::Write;
use std::path::Path;

use semver::Version;
use updater_contract::{
    CurrentState, Layout, STATE_SCHEMA, UPDATES_FILE, UPDATES_SCHEMA, UpdaterError, UpdatesFile,
    VERSION_FILE,
};

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
    let json = serde_json::to_vec_pretty(state).map_err(UpdaterError::invalid)?;
    write_atomic(&layout.current, &json)
}

/// Zapis atomowy: plik tymczasowy obok (`.<nazwa>.tmp`) + `fsync` + `rename`.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), UpdaterError> {
    let dir = path
        .parent()
        .ok_or_else(|| UpdaterError::invalid("ścieżka bez katalogu"))?;
    std::fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .ok_or_else(|| UpdaterError::invalid("ścieżka bez nazwy pliku"))?;
    let tmp = dir.join(format!(".{}.tmp", name.to_string_lossy()));
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

/// `updates.json` (ostatnie sprawdzenie, pokazane „Co nowego”); brak/uszkodzony → domyślny.
pub fn read_updates_file(layout: &Layout) -> UpdatesFile {
    std::fs::read(layout.root.join(UPDATES_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<UpdatesFile>(&b).ok())
        .filter(|f| f.schema == UPDATES_SCHEMA)
        .unwrap_or_default()
}

/// Atomowy zapis `updates.json`.
pub fn write_updates_file(layout: &Layout, file: &UpdatesFile) -> Result<(), UpdaterError> {
    let mut file = file.clone();
    file.schema = UPDATES_SCHEMA;
    let json = serde_json::to_vec_pretty(&file).map_err(UpdaterError::invalid)?;
    write_atomic(&layout.root.join(UPDATES_FILE), &json)
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
