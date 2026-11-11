//! Kontrakt `tools-dev` (docs/modules/tools-dev/SPEC.md) — narzędzia programisty w katalogu
//! roboczym sesji:
//! - `dev_git` — stan, różnice, historia, gałęzie, zmiany zatwierdzenia (tylko odczyt, zgoda
//!   Brokera `fs.read(katalog roboczy)`); zatwierdzanie i wypychanie — przez powłokę z jej zgodami;
//! - `dev_test` — wykrycie rodzaju projektu (cargo, npm/pnpm/yarn, pytest, go, dotnet) i testy
//!   w Job Object z limitem czasu; podsumowanie z liczbami i nazwami nieudanych testów (zgoda
//!   `shell.exec(katalog roboczy)` — testy wykonują kod projektu).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod command;
mod parse;

pub use command::{MAX_ARG, detect_runner, git_command, safe_path, safe_token, test_command};
pub use parse::{Counts, MAX_FAILURES, git_status, test_counts};

use risk_classifier_contract::Reversibility;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::{ToolManifest, schema_of};

/// Domyślny limit testów (s).
pub const TEST_TIMEOUT_DEFAULT_S: u32 = 600;
/// Największy limit testów (s).
pub const TEST_TIMEOUT_MAX_S: u32 = 1_800;

/// Czynność `dev_git`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GitAction {
    /// Gałąź i zmienione pliki.
    Status,
    /// Różnice (robocze albo `staged`, opcjonalnie względem `rev`).
    Diff,
    /// Ostatnie zatwierdzenia.
    Log,
    /// Gałęzie lokalne.
    Branches,
    /// Jedno zatwierdzenie (`rev`, domyślnie HEAD).
    Show,
}

/// `dev_git`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GitArgs {
    /// Czynność.
    pub action: GitAction,
    /// Ścieżka względna (zawężenie do pliku albo katalogu).
    #[serde(default)]
    pub path: Option<String>,
    /// Różnice przygotowane do zatwierdzenia.
    #[serde(default)]
    pub staged: Option<bool>,
    /// Liczba zatwierdzeń w historii (1–200, domyślnie 20).
    #[serde(default)]
    pub limit: Option<u32>,
    /// Rewizja (`HEAD~1`, gałąź, skrót).
    #[serde(default)]
    pub rev: Option<String>,
}

/// Plik w stanie repozytorium.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitFile {
    /// Kod `XY` z `git status` (`M`, `A`, `??`, `D`…).
    pub status: String,
    /// Ścieżka.
    pub path: String,
}

/// Stan repozytorium.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitStatus {
    /// Gałąź.
    pub branch: Option<String>,
    /// Zatwierdzenia przed / za gałęzią zdalną.
    pub ahead: u32,
    /// Zatwierdzenia za gałęzią zdalną.
    pub behind: u32,
    /// Zmienione pliki.
    pub files: Vec<GitFile>,
    /// Brak zmian.
    pub clean: bool,
}

/// Rodzaj projektu (narzędzie testów).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Runner {
    /// `cargo test`.
    Cargo,
    /// `npm test`.
    Npm,
    /// `pnpm test`.
    Pnpm,
    /// `yarn test`.
    Yarn,
    /// `python -m pytest`.
    Pytest,
    /// `go test ./...`.
    Go,
    /// `dotnet test`.
    Dotnet,
}

/// `dev_test`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TestArgs {
    /// Narzędzie (domyślnie wykryte z plików projektu).
    #[serde(default)]
    pub runner: Option<Runner>,
    /// Zawężenie do testów pasujących do nazwy.
    #[serde(default)]
    pub filter: Option<String>,
    /// Limit czasu (s, domyślnie 600, najwyżej 1800).
    #[serde(default)]
    pub timeout_s: Option<u32>,
}

/// Wynik `dev_test`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestSummary {
    /// Narzędzie.
    pub runner: Runner,
    /// Uruchomione polecenie.
    pub command: String,
    /// Zaliczone / niezaliczone / pominięte.
    pub passed: u32,
    /// Niezaliczone.
    pub failed: u32,
    /// Pominięte.
    pub skipped: u32,
    /// Nazwy niezaliczonych testów.
    pub failures: Vec<String>,
    /// Kod wyjścia (`None` — przerwane).
    pub exit_code: Option<i32>,
    /// Przekroczony limit czasu.
    pub timed_out: bool,
    /// Potwierdzone zaliczenie: kod 0, rozpoznane podsumowanie, ≥1 zaliczony i 0 niezaliczonych.
    pub ok: bool,
    /// Czas (ms).
    pub elapsed_ms: u64,
}

fn manifest(
    name: &str,
    title: &str,
    description: &str,
    input: serde_json::Value,
    output: serde_json::Value,
    git: bool,
) -> ToolManifest {
    ToolManifest {
        name: name.into(),
        id: format!("tools-dev.{}", name.trim_start_matches("dev_")),
        title: title.into(),
        description: description.into(),
        input_schema: input,
        output_schema: output,
        reversible: if git {
            Reversibility::Yes
        } else {
            Reversibility::Scoped
        },
        capabilities: vec![if git { "fs.read" } else { "shell.exec" }.into()],
        groups: vec!["shell".into(), "dev".into()],
        mutating: !git,
        untrusted_output: Some(safety_broker_contract::TaintSource::File),
    }
}

/// Manifesty zestawu.
pub fn manifests() -> Vec<ToolManifest> {
    vec![
        manifest(
            "dev_git",
            "Git (odczyt)",
            "Stan repozytorium w katalogu roboczym: `status` (gałąź, zmienione pliki), `diff` (różnice; `staged`, `rev`, `path`), `log` (historia; `limit`), `branches`, `show` (jedno zatwierdzenie). Tylko odczyt — zatwierdzanie i wypychanie przez powłokę.",
            schema_of::<GitArgs>(),
            serde_json::json!({"type": "object"}),
            true,
        ),
        manifest(
            "dev_test",
            "Uruchom testy",
            "Uruchamia testy projektu w katalogu roboczym (rodzaj wykrywany z plików: Cargo.toml, package.json, go.mod, *.csproj, pytest) i zwraca podsumowanie: zaliczone, niezaliczone z nazwami, pominięte, czas. `filter` zawęża do testów o nazwie. Wymaga zgody Sędziego Cieni — testy wykonują kod projektu.",
            schema_of::<TestArgs>(),
            schema_of::<TestSummary>(),
            false,
        ),
    ]
}

/// Sprawdza argumenty narzędzia (ta sama walidacja w `-impl` i `-fake`).
pub fn check_args(tool: &str, args: &serde_json::Value) -> Result<(), String> {
    let e = |e: serde_json::Error| e.to_string();
    match tool {
        "dev_git" => {
            let a: GitArgs = serde_json::from_value(args.clone()).map_err(e)?;
            git_command(&a).map(|_| ())
        }
        "dev_test" => {
            let a: TestArgs = serde_json::from_value(args.clone()).map_err(e)?;
            if let Some(f) = &a.filter {
                safe_token(f)?;
            }
            if a.timeout_s
                .is_some_and(|t| t == 0 || t > TEST_TIMEOUT_MAX_S)
            {
                return Err(format!("limit czasu 1–{TEST_TIMEOUT_MAX_S} s"));
            }
            Ok(())
        }
        other => Err(format!("nieznane narzędzie {other}")),
    }
}

/// Przykładowe poprawne argumenty.
pub fn sample_args(tool: &str) -> serde_json::Value {
    match tool {
        "dev_git" => serde_json::json!({"action": "status"}),
        _ => serde_json::json!({}),
    }
}

/// Testy kontraktowe zestawu (feature `contract-tests`).
#[cfg(feature = "contract-tests")]
pub mod contract_tests {
    use std::sync::Arc;

    use tools_common_contract::{Tool, contract_tests as common};

    use super::{manifests, sample_args};

    /// Manifesty, odrzucanie złych argumentów, wstrzyknięcia w argumentach odrzucone.
    pub async fn run_all(tools: &[Arc<dyn Tool>]) {
        assert_eq!(tools.len(), manifests().len());
        for m in manifests() {
            let tool = tools
                .iter()
                .find(|t| t.manifest().name == m.name)
                .unwrap_or_else(|| panic!("brak narzędzia {}", m.name));
            assert_eq!(tool.manifest(), &m);
            common::manifest_is_valid(tool.as_ref());
            common::invalid_args_rejected(tool.as_ref(), "/").await;
            let _ = sample_args(&m.name);
        }
        let git = tools
            .iter()
            .find(|t| t.manifest().name == "dev_git")
            .unwrap_or_else(|| panic!("dev_git"));
        for bad in [
            serde_json::json!({"action": "log", "rev": "--output=x"}),
            serde_json::json!({"action": "diff", "path": "../../Windows"}),
            serde_json::json!({"action": "show", "rev": "HEAD & del x"}),
        ] {
            assert!(
                !git.call(bad.clone(), &common::ctx("/")).await.is_ok(),
                "{bad}"
            );
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
