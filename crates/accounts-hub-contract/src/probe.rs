//! Porty zewnętrzne kreatora: test połączenia, wykrywanie modeli, zmienne środowiskowe,
//! wykrywanie mostów CLI (tylko istnienie w PATH i wersja z `--version`).

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::account::ConnectionReport;
use crate::catalog::{ModelInfo, ProviderCatalogEntry};
use crate::secret::SecretString;

/// Parametry testu połączenia / wykrywania modeli.
#[derive(Debug, Clone, Copy)]
pub struct ConnectionRequest<'a> {
    /// Dostawca z katalogu.
    pub provider: &'a ProviderCatalogEntry,
    /// Endpoint (z konta albo katalogu).
    pub base_url: Option<&'a str>,
    /// Klucz (brak dla `auth = none`).
    pub secret: Option<&'a SecretString>,
}

/// Test połączenia z dostawcą (produkcyjnie adapter z `providers-*`; tu tylko kontrakt).
#[async_trait]
pub trait ConnectionTester: Send + Sync {
    /// Sprawdza połączenie i uwierzytelnienie. Komunikaty nie mogą zawierać klucza.
    async fn test(&self, request: ConnectionRequest<'_>) -> ConnectionReport;
}

/// Błędy wykrywania modeli.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum ModelListError {
    /// Dostawca nie udostępnia listy modeli — użytkownik wpisze identyfikator ręcznie.
    #[error("dostawca nie udostępnia listy modeli")]
    Unsupported,
    /// Klucz odrzucony.
    #[error("klucz odrzucony przy pobieraniu listy modeli")]
    InvalidKey,
    /// Błąd sieci.
    #[error("błąd sieci: {message}")]
    Network {
        /// Opis (bez sekretów).
        message: String,
    },
    /// Przekroczony czas.
    #[error("przekroczony czas pobierania listy modeli")]
    Timeout,
}

/// Wykrywanie modeli i możliwości (np. Models API).
#[async_trait]
pub trait ModelLister: Send + Sync {
    /// Lista modeli dostępnych dla klucza.
    async fn list_models(
        &self,
        request: ConnectionRequest<'_>,
    ) -> Result<Vec<ModelInfo>, ModelListError>;
}

/// Źródło zmiennych środowiskowych (import kluczy na życzenie użytkownika).
pub trait EnvSource: Send + Sync {
    /// Wartość zmiennej jako sekret; `None`, gdy brak lub pusta.
    fn var(&self, name: &str) -> Option<SecretString>;
}

/// Mosty CLI wykrywane w F1 (kolejne — Grok Build, Kimi, agy — od F4).
pub const KNOWN_CLI_BRIDGES: [&str; 2] = ["claude", "codex"];

/// Wykryty most CLI: tylko nazwa, ścieżka programu i wersja. Alfa nie czyta katalogów
/// konfiguracji ani tokenów CLI — logowanie wykonuje użytkownik w terminalu ConPTY (F4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CliBridge {
    /// Nazwa programu (`claude`, `codex`).
    pub name: String,
    /// Pełna ścieżka pliku wykonywalnego znalezionego w PATH.
    pub path: PathBuf,
    /// Wersja z `--version` (pierwszy token w formacie `x.y.z`), jeśli udało się ją odczytać.
    pub version: Option<String>,
}

/// Port wykrywania CLI.
pub trait CliProbe: Send + Sync {
    /// Szuka programu w PATH (z rozszerzeniami PATHEXT na Windows).
    fn locate(&self, program: &str) -> Option<PathBuf>;

    /// Uruchamia `<program> --version` z limitem czasu i zwraca standardowe wyjście.
    fn version_output(&self, path: &Path) -> Option<String>;
}

/// Wykrywa znane mosty CLI przez podany port.
pub fn detect_cli_bridges_with(probe: &dyn CliProbe) -> Vec<CliBridge> {
    KNOWN_CLI_BRIDGES
        .iter()
        .filter_map(|name| {
            let path = probe.locate(name)?;
            let version = probe
                .version_output(&path)
                .as_deref()
                .and_then(parse_version);
            Some(CliBridge {
                name: (*name).to_owned(),
                path,
                version,
            })
        })
        .collect()
}

/// Pierwszy token w formacie `x.y[.z…]` z opcjonalnym sufiksem (`-beta.1`), np. z
/// `2.1.0 (Claude Code)` albo `codex-cli 0.46.0`.
pub fn parse_version(output: &str) -> Option<String> {
    output
        .split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')')
        .map(|t| t.trim_start_matches(['v', 'V']))
        .find(|t| {
            let core = t.split(['-', '+']).next().unwrap_or_default();
            let parts: Vec<&str> = core.split('.').collect();
            parts.len() >= 2
                && parts
                    .iter()
                    .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        })
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(
            parse_version("2.1.0 (Claude Code)").as_deref(),
            Some("2.1.0")
        );
        assert_eq!(
            parse_version("codex-cli 0.46.0\n").as_deref(),
            Some("0.46.0")
        );
        assert_eq!(
            parse_version("v1.2.3-beta.1").as_deref(),
            Some("1.2.3-beta.1")
        );
        assert_eq!(parse_version("no version here"), None);
        assert_eq!(parse_version("1."), None);
    }

    struct Probe;

    impl CliProbe for Probe {
        fn locate(&self, program: &str) -> Option<PathBuf> {
            (program == "claude").then(|| PathBuf::from("/bin/claude"))
        }

        fn version_output(&self, _path: &Path) -> Option<String> {
            Some("2.0.1 (Claude Code)".into())
        }
    }

    #[test]
    fn detects_only_present_bridges() {
        let found = detect_cli_bridges_with(&Probe);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "claude");
        assert_eq!(found[0].version.as_deref(), Some("2.0.1"));
    }
}
