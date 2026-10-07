//! Specyfikacja wyzwalacza: rodzaj (czas, zdarzenie, ręczny), właściciel, akcja (zadanie dla
//! schedulera), sufit uprawnień, limit częstości, okno ciszy, zaległe uruchomienia.

use std::fmt;

use core_bus_contract::SessionId;
use personas_contract::PersonaId;
use safety_broker_contract::Capability;
use scheduler_contract::{Assignee, ExecutorKind, Resource, TaskBudget, TaskClass};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::cron::CronExpr;
use crate::tz::Tz;

/// Identyfikator wyzwalacza: 1–64 znaki `[a-z0-9._-]`.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct TriggerId(pub String);

impl TriggerId {
    /// Identyfikator z tekstu.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Poprawna postać.
    pub fn is_valid(&self) -> bool {
        (1..=64).contains(&self.0.len())
            && self.0.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
            })
    }
}

impl From<&str> for TriggerId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Display for TriggerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Kto działa (tworzy, zmienia, uruchamia ręcznie) albo jest właścicielem.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Actor {
    /// Użytkownik (UI, głos) — jedyny, kto może dopuścić mosty w harmonogramie.
    User,
    /// Agentka (np. Kreator agentów: `triggers: [{ cron: … }]` w manifeście).
    Agent(PersonaId),
    /// Usługa systemowa.
    System(String),
}

/// Filtr wyniku zakończonego zadania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FinishFilter {
    /// Tylko sukces.
    Succeeded,
    /// Tylko niepowodzenie (błąd, budżet, termin).
    Failed,
    /// Dowolne zakończenie.
    Any,
}

/// Rodzaj wyzwalacza.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TriggerKind {
    /// Cron w strefie wyzwalacza (DST: patrz [`CronExpr`]).
    Cron {
        /// Wyrażenie.
        expr: CronExpr,
    },
    /// Jednorazowo (ms UTC).
    Once {
        /// Chwila.
        at_ms: u64,
    },
    /// Co `every_ms` od `start_ms` (czas absolutny — bez wpływu DST).
    Interval {
        /// Odstęp (≥ 60 s).
        every_ms: u64,
        /// Początek (domyślnie chwila utworzenia).
        #[serde(default)]
        start_ms: Option<u64>,
    },
    /// Nowy plik w katalogu (przez port obserwacji plików platformy).
    FileInDir {
        /// Katalog.
        dir: String,
        /// Wzorzec nazwy (`*`, `?`), np. `*.pdf`.
        #[serde(default)]
        pattern: Option<String>,
    },
    /// Nowa wiadomość w sesji (`session.turn.appended`, nie od asystentki).
    NewMessage {
        /// Tylko ta sesja.
        #[serde(default)]
        session: Option<SessionId>,
    },
    /// Koniec zadania schedulera (`scheduler.task.finished`).
    TaskFinished {
        /// Prefiks identyfikatora zadania.
        #[serde(default)]
        task_prefix: Option<String>,
        /// Filtr wyniku.
        #[serde(default = "any")]
        outcome: FinishFilter,
    },
    /// Tylko ręcznie („Uruchom teraz”).
    Manual,
}

fn any() -> FinishFilter {
    FinishFilter::Any
}

impl TriggerKind {
    /// Czy wyzwalacz czasowy.
    pub fn is_time(&self) -> bool {
        matches!(
            self,
            Self::Cron { .. } | Self::Once { .. } | Self::Interval { .. }
        )
    }

    /// Krótka nazwa.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cron { .. } => "cron",
            Self::Once { .. } => "once",
            Self::Interval { .. } => "interval",
            Self::FileInDir { .. } => "file_in_dir",
            Self::NewMessage { .. } => "new_message",
            Self::TaskFinished { .. } => "task_finished",
            Self::Manual => "manual",
        }
    }
}

/// Co zrobić po wyzwoleniu: zadanie dla schedulera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TriggerAction {
    /// Tytuł zadania (Oś czasu).
    pub title: String,
    /// Cel (polecenie dla agentki; treść wyzwalająca idzie osobno jako niezaufana).
    pub goal: String,
    /// Przydział.
    pub assignee: Assignee,
    /// Klasa (najwyżej `Agent` — wyzwalacz nie jest żądaniem użytkownika).
    #[serde(default = "background")]
    pub class: TaskClass,
    /// Wykonawca (most CLI — nigdy z wyzwalacza).
    #[serde(default)]
    pub executor: ExecutorKind,
    /// Zasoby wyłączne.
    #[serde(default)]
    pub resources: Vec<Resource>,
    /// Budżety zadania.
    #[serde(default)]
    pub budget: TaskBudget,
    /// Termin zadania względem wyzwolenia (ms; domyślnie 1 h).
    #[serde(default)]
    pub deadline_after_ms: Option<u64>,
    /// Tylko w bezczynności użytkownika.
    #[serde(default)]
    pub only_when_idle: bool,
}

fn background() -> TaskClass {
    TaskClass::Background
}

impl TriggerAction {
    /// Akcja tła dla dowolnej agentki.
    pub fn new(title: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            goal: goal.into(),
            assignee: Assignee::AnyAgent,
            class: TaskClass::Background,
            executor: ExecutorKind::Agent,
            resources: Vec::new(),
            budget: TaskBudget::default(),
            deadline_after_ms: None,
            only_when_idle: false,
        }
    }
}

/// Limit częstości: najwyżej `max_fires` w oknie `per_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RateLimit {
    /// Najwięcej uruchomień.
    pub max_fires: u32,
    /// Okno (ms).
    pub per_ms: u64,
}

impl Default for RateLimit {
    /// 12 na godzinę.
    fn default() -> Self {
        Self {
            max_fires: 12,
            per_ms: 3_600_000,
        }
    }
}

/// Co robić w oknie ciszy / DND.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuietMode {
    /// Odłożyć do końca ciszy (jedno uruchomienie zbiorcze).
    #[default]
    Defer,
    /// Pominąć (zapis w dzienniku).
    Skip,
}

/// Okno ciszy w czasie lokalnym strefy wyzwalacza (`start > end` = przez północ).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuietHours {
    /// Początek (minuta doby 0–1439).
    pub start_min: u16,
    /// Koniec (minuta doby 0–1439, wyłącznie).
    pub end_min: u16,
    /// Dni tygodnia (0 = niedziela); puste = codziennie.
    #[serde(default)]
    pub days: Vec<u8>,
}

/// Co z wystąpieniami czasowymi przegapionymi (program wyłączony, uśpienie).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MisfirePolicy {
    /// Jedno uruchomienie zbiorcze po powrocie.
    #[default]
    FireOnce,
    /// Pominąć (zapis w dzienniku).
    Skip,
}

/// Specyfikacja wyzwalacza.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TriggerSpec {
    /// Identyfikator.
    pub id: TriggerId,
    /// Nazwa (UI).
    pub name: String,
    /// Właściciel (musi być tym, kto tworzy).
    pub owner: Actor,
    /// Rodzaj.
    pub kind: TriggerKind,
    /// Strefa czasowa (cron, okno ciszy).
    #[serde(default)]
    pub tz: Tz,
    /// Akcja.
    pub action: TriggerAction,
    /// Sufit uprawnień zadania — tokeny wydaje Broker **przy wykonaniu** (nie przy tworzeniu).
    #[serde(default)]
    pub scope: Vec<Capability>,
    /// Limit częstości.
    #[serde(default)]
    pub rate: RateLimit,
    /// Okno ciszy.
    #[serde(default)]
    pub quiet: Option<QuietHours>,
    /// Co w ciszy / DND.
    #[serde(default)]
    pub quiet_mode: QuietMode,
    /// Czy respektować globalne „Nie przeszkadzać”.
    #[serde(default = "yes")]
    pub respect_dnd: bool,
    /// Zaległe uruchomienia.
    #[serde(default)]
    pub misfire: MisfirePolicy,
    /// Dawna zgoda harmonogramu na mosty CLI — tylko dla zgodności odczytu zapisanych wyzwalaczy;
    /// `true` jest odrzucane przy tworzeniu/zmianie (AGENTS.md: mostów nie uruchamia się
    /// z harmonogramu; CX-d).
    #[serde(default)]
    pub allow_bridges: bool,
    /// Włączony.
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

impl TriggerSpec {
    /// Wyzwalacz z wartościami domyślnymi (strefa Europe/Warsaw, limit 12/h, cisza: brak).
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        owner: Actor,
        kind: TriggerKind,
        action: TriggerAction,
    ) -> Self {
        Self {
            id: TriggerId(id.into()),
            name: name.into(),
            owner,
            kind,
            tz: Tz::warsaw(),
            action,
            scope: Vec::new(),
            rate: RateLimit::default(),
            quiet: None,
            quiet_mode: QuietMode::Defer,
            respect_dnd: true,
            misfire: MisfirePolicy::FireOnce,
            allow_bridges: false,
            enabled: true,
        }
    }
}
