//! Błędy backendu agentowego. Komunikaty nie zawierają treści promptów, tokenów ani zmiennych środowiska.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Powód odmowy uruchomienia mostu ze względu na pochodzenie żądania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum LaunchRefusal {
    /// Ulepszacz nigdy nie uruchamia mostów (PLAN §1.3 pkt 4, §12).
    Improver,
    /// Wyzwalacze nie uruchamiają mostów (subscription-routes.md §2.4).
    Trigger,
    /// Harmonogram bez jawnej zgody użytkownika dla tej trasy.
    ScheduleWithoutConsent,
    /// Wyczerpany dzienny limit uruchomień z harmonogramu.
    ScheduleDailyLimit {
        /// Limit.
        limit: u32,
    },
}

/// Błąd backendu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum BackendError {
    /// Most nie startuje z tego źródła.
    #[error("most nie może zostać uruchomiony z tego źródła: {refusal:?}")]
    LaunchRefused {
        /// Powód.
        refusal: LaunchRefusal,
    },
    /// Trasa wyłączona/zabroniona/niedozwolona dla sesji (rejestr zgodności).
    #[error("trasa `{route}` niedozwolona: {reason}")]
    RouteNotAllowed {
        /// Trasa.
        route: String,
        /// Powód z `compliance` (stabilny tekst dla UI).
        reason: String,
    },
    /// Wersja CLI spoza listy przypiętych (albo nieodczytana) — trasa wyłączona.
    #[error("wersja CLI `{program}` {found:?} nie jest przypięta — trasa wyłączona")]
    VersionNotPinned {
        /// Program.
        program: String,
        /// Znaleziona wersja.
        found: Option<String>,
    },
    /// Hash pliku wykonywalnego CLI nie zgadza się z przypiętym.
    #[error("plik wykonywalny `{program}` różni się od przypiętego (hash)")]
    BinaryHashMismatch {
        /// Program.
        program: String,
    },
    /// Nie znaleziono CLI.
    #[error("nie znaleziono programu `{program}`")]
    CliNotFound {
        /// Program.
        program: String,
    },
    /// Most nie jest skonfigurowany w tym backendzie.
    #[error("most `{0}` nie jest skonfigurowany")]
    BridgeUnavailable(String),
    /// Niepoprawna specyfikacja zadania.
    #[error("niepoprawne zadanie: {0}")]
    InvalidSpec(String),
    /// Nieznane zadanie.
    #[error("nieznane zadanie `{0}`")]
    UnknownTask(String),
    /// Nieznana (albo już rozstrzygnięta) prośba o uprawnienie.
    #[error("nieznana prośba o uprawnienie `{0}`")]
    UnknownPermissionRequest(String),
    /// Zadanie już zakończone.
    #[error("zadanie już zakończone")]
    TaskFinished,
    /// Steering niedostępny w tym stanie (np. wejście CLI zamknięte).
    #[error("sterowanie niedostępne: {0}")]
    SteeringUnavailable(String),
    /// Błąd przygotowania katalogu roboczego (worktree/kopia).
    #[error("katalog roboczy: {0}")]
    Workspace(String),
    /// Nie udało się uruchomić procesu.
    #[error("nie udało się uruchomić CLI: {0}")]
    Spawn(String),
    /// Naruszenie protokołu strumienia CLI, którego nie da się pominąć.
    #[error("protokół CLI: {0}")]
    Protocol(String),
    /// Proces CLI zakończył się bez wyniku (crash) albo z kodem ≠ 0.
    #[error("CLI zakończyło się kodem {code:?} bez wyniku")]
    CliExited {
        /// Kod wyjścia (brak = sygnał).
        code: Option<i32>,
        /// Ostatnie linie stderr (skrócone, bez zmiennych środowiska).
        stderr_tail: String,
    },
    /// CLI zgłosiło błąd (np. odrzucone uwierzytelnienie, limit planu).
    #[error("CLI zgłosiło błąd: {0}")]
    CliReported(String),
    /// Zadanie anulowane.
    #[error("zadanie anulowane")]
    Cancelled,
    /// Przekroczony budżet.
    #[error("przekroczony budżet: {0}")]
    BudgetExceeded(String),
    /// Błąd kanału MCP Alfy (rejestracja tokenu).
    #[error("kanał MCP Alfy: {0}")]
    Mcp(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_tagged() {
        let e = BackendError::LaunchRefused {
            refusal: LaunchRefusal::Trigger,
        };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["error"], "launch_refused");
        assert_eq!(json["refusal"]["refusal"], "trigger");
        assert!(e.to_string().contains("most"));
        let back: BackendError = serde_json::from_value(json).unwrap();
        assert_eq!(back, e);
    }
}
