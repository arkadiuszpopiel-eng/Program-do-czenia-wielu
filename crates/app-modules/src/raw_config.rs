//! Ustawienia → Zaawansowane: surowy edytor plików konfiguracji (`shared.toml` i nakładka
//! maszyny). Zapis: kontrola składni (z numerem wiersza), zapis atomowy, przeładowanie przez
//! magazyn konfiguracji z pełną walidacją (schematy modułów, zakaz kluczy Jądra `kernel.*`
//! i sekretów); odmowa → poprzednia treść pliku wraca, a użytkownik dostaje powód.

use std::path::{Path, PathBuf};

use app_api::AppError;
use app_api::dto::RawConfig;
use core_config_impl::{FileConfigStore, write_atomic};

/// Największy plik w edytorze.
pub const MAX_BYTES: usize = 256 * 1024;

/// Warstwa pliku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawLayer {
    /// Wspólna dla maszyn (`shared.toml`).
    Shared,
    /// Nakładka tej maszyny (`machine/<id>.toml`).
    Machine,
}

impl RawLayer {
    /// Z nazwy (`shared`, `machine`).
    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "shared" => Ok(Self::Shared),
            "machine" => Ok(Self::Machine),
            other => Err(AppError::invalid_field(
                "layer",
                format!("nieznana warstwa `{other}`"),
            )),
        }
    }
}

/// Ścieżka pliku warstwy (te same nazwy co w `core-config-impl`).
pub fn path(dir: &Path, machine: &str, layer: RawLayer) -> PathBuf {
    match layer {
        RawLayer::Shared => dir.join("shared.toml"),
        RawLayer::Machine => dir.join("machine").join(format!("{machine}.toml")),
    }
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

/// Treść obu plików i problemy: składnia każdego pliku sprawdzana teraz, a problemy z walidacji
/// przy starcie programu — tylko dopóki plik się nie zmienił od startu (po naprawie w edytorze
/// stary wpis znika). Magazyn konfiguracji to Jądro, więc jego stanu tu nie zmieniamy.
pub fn get(store: &FileConfigStore, machine: &str) -> RawConfig {
    let dir = store.dir();
    let shared = path(dir, machine, RawLayer::Shared);
    let own = path(dir, machine, RawLayer::Machine);
    let (shared_text, own_text) = (read(&shared), read(&own));
    let mut problems: Vec<String> = Vec::new();
    for (name, text) in [
        ("shared.toml", &shared_text),
        ("nakładka maszyny", &own_text),
    ] {
        if let Err(e) = check(text) {
            problems.push(format!("{name}: {e}"));
        }
    }
    if problems.is_empty()
        && !store.load_problems().is_empty()
        && !edited_since_start(&[&shared, &own])
    {
        problems.extend(store.load_problems().iter().cloned());
    }
    RawConfig {
        shared: shared_text,
        machine: own_text,
        shared_path: shared.display().to_string(),
        machine_path: own.display().to_string(),
        problems,
    }
}

/// Czy któryś plik zmienił się po starcie procesu (wtedy problemy ze startu są nieaktualne).
fn edited_since_start(paths: &[&Path]) -> bool {
    static STARTED: std::sync::OnceLock<std::time::SystemTime> = std::sync::OnceLock::new();
    let started = *STARTED.get_or_init(std::time::SystemTime::now);
    paths.iter().any(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .is_ok_and(|t| t > started)
    })
}

/// Kontrola składni TOML: liczba kluczy (płasko) albo błąd z wierszem.
pub fn check(text: &str) -> Result<usize, String> {
    if text.len() > MAX_BYTES {
        return Err(format!("plik większy niż {} KB", MAX_BYTES / 1024));
    }
    let table: toml::Table = toml::from_str(text).map_err(|e| {
        let line = e
            .span()
            .map(|s| text[..s.start.min(text.len())].lines().count().max(1));
        match line {
            Some(l) => format!("wiersz {l}: {}", e.message()),
            None => e.message().to_owned(),
        }
    })?;
    fn count(v: &toml::Value) -> usize {
        match v {
            toml::Value::Table(t) => t.values().map(count).sum(),
            _ => 1,
        }
    }
    Ok(table.values().map(count).sum())
}

/// Zapis warstwy z walidacją magazynu; przy odmowie przywraca poprzednią treść.
pub async fn save(
    store: &FileConfigStore,
    machine: &str,
    layer: RawLayer,
    text: &str,
) -> Result<usize, AppError> {
    let keys = check(text)
        .map_err(|e| AppError::invalid_field("text", format!("Błąd składni TOML: {e}.")))?;
    let p = path(store.dir(), machine, layer);
    let before = std::fs::read_to_string(&p).ok();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(AppError::storage)?;
    }
    write_atomic(&p, text).map_err(AppError::storage)?;
    match store.reload().await {
        Ok(_) => Ok(keys),
        Err(e) => {
            let restored = match &before {
                Some(old) => write_atomic(&p, old),
                None => std::fs::remove_file(&p),
            };
            let _ = store.reload().await;
            if let Err(r) = restored {
                tracing::error!("nie przywrócono pliku konfiguracji po odmowie: {r}");
            }
            Err(AppError::invalid_field(
                "text",
                format!("Konfiguracja odrzucona, plik przywrócony: {e}."),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_check_counts_keys_and_reports_line() {
        assert_eq!(
            check("[ui]\ntheme = \"dark\"\nscale = 1.2\n[voice]\nmic = true\n"),
            Ok(3)
        );
        let err = check("[ui]\ntheme = \"dark\"\nzla linia\n").unwrap_err();
        assert!(err.starts_with("wiersz 3"), "{err}");
        assert!(check(&"a".repeat(MAX_BYTES + 1)).is_err());
        assert_eq!(RawLayer::parse("machine").unwrap(), RawLayer::Machine);
        assert!(RawLayer::parse("kernel").is_err());
    }
}
