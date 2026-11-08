//! Historia zmian: append-only `history.ndjson` (klucz, stara, nowa, źródło).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Utc};
use core_config_contract::{ConfigKey, ConfigLayer, ConfigValue, Origin, Scope};
use serde::{Deserialize, Serialize};

/// Skąd przyszła zmiana.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSource {
    /// Wywołanie `ConfigStore::set`.
    Api,
    /// Przeładowanie po zmianie pliku (edycja ręczna/zewnętrzna).
    File,
}

/// Wpis historii — surowa zmiana jednej warstwy (nie wartości wynikowej).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Czas zmiany.
    pub ts: DateTime<Utc>,
    /// Klucz.
    pub key: ConfigKey,
    /// Zakres.
    pub scope: Scope,
    /// Warstwa (plik).
    pub layer: ConfigLayer,
    /// Wartość przed zmianą w tej warstwie.
    pub old: Option<ConfigValue>,
    /// Wartość po zmianie (`None` = usunięto).
    pub new: Option<ConfigValue>,
    /// Inicjator.
    pub origin: Origin,
    /// Źródło zmiany.
    pub source: ChangeSource,
}

/// Dopisuje wpisy (jedna linia JSON na wpis).
pub fn append(path: &Path, entries: &[HistoryEntry]) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }
    let mut text = String::new();
    for entry in entries {
        text.push_str(&serde_json::to_string(entry).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.flush())
        .map_err(|e| e.to_string())
}

/// Czyta wpisy (linie uszkodzone pomija); brak pliku = pusta historia.
pub fn read(path: &Path) -> Result<Vec<HistoryEntry>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn append_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.ndjson");
        assert!(read(&path).unwrap().is_empty());
        let entry = HistoryEntry {
            ts: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
            key: ConfigKey::new("a.b").unwrap(),
            scope: Scope::Global,
            layer: ConfigLayer::Shared,
            old: None,
            new: Some(json!(1)),
            origin: Origin::User,
            source: ChangeSource::Api,
        };
        append(&path, std::slice::from_ref(&entry)).unwrap();
        append(&path, &[]).unwrap();
        std::fs::write(
            &path,
            format!("{}\nuszkodzona\n", serde_json::to_string(&entry).unwrap()),
        )
        .unwrap();
        assert_eq!(read(&path).unwrap(), vec![entry]);
    }
}
