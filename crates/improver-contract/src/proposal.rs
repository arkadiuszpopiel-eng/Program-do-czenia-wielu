//! Propozycje Ulepszacza, etapy potoku, zatwierdzenia, sygnały wejściowe.

use std::collections::BTreeMap;
use std::fmt;

use evals_contract::{GateVerdict, SuiteId, sha256_hex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::guard::{ChangeTarget, Violation};
use crate::ring::{Ring, SafetyClass};

/// Identyfikator propozycji.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ProposalId(pub u64);

impl fmt::Display for ProposalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P-{}", self.0)
    }
}

/// Zestaw zmian zaproponowany przez regułę albo model (niezaufany do czasu oceny strażnika).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CandidateSet {
    /// Tytuł (po polsku).
    pub title: String,
    /// Uzasadnienie (retrospektywa).
    pub rationale: String,
    /// Źródło (`rule:…`, `model:…`).
    pub source: String,
    /// Zmiany.
    pub targets: Vec<ChangeTarget>,
}

/// Zmiana zaplanowana (po ocenie strażnika).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlannedChange {
    /// Klucz konfiguracji.
    pub key: String,
    /// Wartość wynikowa przed zmianą (`None` = brak).
    pub old: Option<Value>,
    /// Nowa wartość.
    pub new: Value,
    /// Pierścień.
    pub ring: Ring,
    /// Klasa bezpieczeństwa.
    pub safety: SafetyClass,
}

/// Etap propozycji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum Stage {
    /// Utworzona, czeka na ocenę.
    Proposed,
    /// Odrzucona w piaskownicy (podział `test`).
    SandboxFailed {
        /// Powód.
        reason: String,
    },
    /// Odrzucona na holdoucie (bramka Jądra).
    HoldoutFailed {
        /// Powód.
        reason: String,
    },
    /// Przeszła bramkę; czeka na zatwierdzenie użytkownika.
    AwaitingApproval,
    /// Wdrożona (przez `core-config`).
    Deployed {
        /// Automatycznie (tylko R0 zawężające/bezpieczne).
        auto: bool,
    },
    /// Okres nadzoru minął bez regresji (nadal cofalna ręcznie).
    Settled,
    /// Cofnięta.
    RolledBack {
        /// Powód.
        reason: String,
        /// Automatycznie (regresja).
        auto: bool,
    },
    /// Odrzucona przez użytkownika.
    Rejected,
    /// Wdrożenie przerwane (wartość zmieniona w międzyczasie albo błąd zapisu); nic nie zostało zmienione.
    Aborted {
        /// Powód.
        reason: String,
    },
}

/// Wpis historii etapów (append-only w obrębie propozycji).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StageNote {
    /// Czas (ms).
    pub ts_ms: u64,
    /// Etap.
    pub stage: Stage,
    /// Notatka.
    pub note: String,
}

/// Propozycja zmiany (karta w panelu „Zdrowie systemu” — diff, ryzyko, plan cofnięcia).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Proposal {
    /// Identyfikator.
    pub id: ProposalId,
    /// Utworzono (ms).
    pub created_ms: u64,
    /// Tytuł.
    pub title: String,
    /// Uzasadnienie.
    pub rationale: String,
    /// Źródło.
    pub source: String,
    /// Najwyższy pierścień zmian.
    pub ring: Ring,
    /// Najmniej bezpieczna klasa zmian.
    pub safety: SafetyClass,
    /// Czy wdrożenie automatyczne jest dopuszczalne (R0, zawężające/bezpieczne).
    pub auto_eligible: bool,
    /// Zmiany.
    pub changes: Vec<PlannedChange>,
    /// SHA-256 zmian (zatwierdzenie dotyczy dokładnie tego diffu).
    pub digest: String,
    /// Zestaw ewaluacyjny bramki.
    pub suite: SuiteId,
    /// Bieżący etap.
    pub stage: Stage,
    /// Wynik piaskownicy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<GateVerdict>,
    /// Wynik holdoutu (zbiorczy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holdout: Option<GateVerdict>,
    /// Wdrożono (ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployed_ms: Option<u64>,
    /// Metryki w chwili wdrożenia (punkt odniesienia nadzoru).
    #[serde(default)]
    pub baseline_metrics: BTreeMap<String, f64>,
    /// Historia etapów.
    pub history: Vec<StageNote>,
}

impl Proposal {
    /// SHA-256 kanonicznego JSON zmian (klucz, stara, nowa wartość).
    pub fn digest_of(changes: &[PlannedChange]) -> String {
        let canon: Vec<(&str, &Option<Value>, &Value)> = changes
            .iter()
            .map(|c| (c.key.as_str(), &c.old, &c.new))
            .collect();
        sha256_hex(&serde_json::to_vec(&canon).unwrap_or_default())
    }

    /// Diff w postaci linii `- klucz = stara` / `+ klucz = nowa`.
    pub fn diff_lines(&self) -> Vec<String> {
        let show = |v: &Option<Value>| {
            v.as_ref()
                .map_or_else(|| "(brak)".to_owned(), Value::to_string)
        };
        self.changes
            .iter()
            .flat_map(|c| {
                [
                    format!("- {} = {}", c.key, show(&c.old)),
                    format!("+ {} = {}", c.key, c.new),
                ]
            })
            .collect()
    }

    /// Plan cofnięcia (po polsku).
    pub fn rollback_plan(&self) -> Vec<String> {
        self.changes
            .iter()
            .rev()
            .map(|c| match &c.old {
                Some(v) => format!("przywróć {} = {v} (core-config, origin = improver)", c.key),
                None => format!("usuń nadpisanie {} (powrót do wartości domyślnej)", c.key),
            })
            .collect()
    }

    /// Czy wdrożona i nadal aktywna (nadzór, rollback).
    pub fn is_live(&self) -> bool {
        matches!(self.stage, Stage::Deployed { .. } | Stage::Settled)
    }
}

/// Zatwierdzenie użytkownika (z UI/Broker-UI; R1–R2 podpisane kluczem TPM — weryfikuje port).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UserApproval {
    /// Propozycja.
    pub proposal: ProposalId,
    /// Hash zatwierdzanego diffu (musi być równy [`Proposal::digest`]).
    pub digest: String,
    /// Skąd (np. `zdrowie-systemu`).
    pub surface: String,
    /// Podpis (R1–R2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Zablokowana próba (dziennik dla panelu i audytu; bez treści wartości).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BlockedAttempt {
    /// Czas (ms).
    pub ts_ms: u64,
    /// Źródło propozycji.
    pub source: String,
    /// Cel (`config:klucz`, `file:ścieżka`, `code:ścieżka`).
    pub target: String,
    /// Naruszenie.
    pub violation: Violation,
}

/// Szkic zgłoszenia R3 (kod) dla sesji deweloperskiej — Ulepszacz nie zmienia kodu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssueDraft {
    /// Czas (ms).
    pub ts_ms: u64,
    /// Tytuł.
    pub title: String,
    /// Plik.
    pub path: String,
    /// Uzasadnienie i łatka (do przeglądu człowieka).
    pub body: String,
}

/// Obserwacja (sygnał retrospektywy).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Observation {
    /// Użytkownik poprawiał wymowę słowa.
    PronunciationFix {
        /// Słowo.
        word: String,
        /// Zapis fonetyczny.
        phonetic: String,
        /// Liczba poprawek.
        count: u32,
    },
    /// Skuteczność trasy routera dla klasy zadań.
    RouteOutcome {
        /// Trasa.
        route: String,
        /// Klasa zadań.
        class: String,
        /// Odsetek sukcesów.
        success_rate: f64,
        /// Liczba prób.
        n: u32,
    },
    /// Powtarzalny przepływ (kandydat na umiejętność, R1).
    RepeatedFlow {
        /// Nazwa.
        name: String,
        /// Kroki.
        steps: Vec<String>,
        /// Ile razy.
        count: u32,
    },
    /// Fałszywe przerwania na godzinę (po filtrze pewności AEC).
    FalseInterruptions {
        /// Na godzinę.
        per_hour: f64,
    },
}

/// Migawka metryk i obserwacji.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MetricsSnapshot {
    /// Czas (ms).
    pub ts_ms: u64,
    /// Metryka → wartość.
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    /// Obserwacje.
    #[serde(default)]
    pub observations: Vec<Observation>,
}

/// Warunki pracy (Ulepszacz tylko w bezczynności, nie na baterii, nie w grze — PLAN §12.4, F9-04).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunConditions {
    /// Zasilanie z baterii.
    pub on_battery: bool,
    /// Tryb gry / pełny ekran.
    pub game_mode: bool,
    /// Użytkownik bezczynny.
    pub user_idle: bool,
}
