//! Specyfikacja zadania: węzeł DAG (zależności z warunkami), przydział, klasa priorytetu,
//! pochodzenie (zgodność mostów), zasoby wyłączne, okno czasowe, budżety i ponowienia.

use agent_backends_contract::{BridgeKind, LaunchOrigin};
use core_bus_contract::SessionId;
use personas_contract::{PersonaId, RoleId};
use safety_broker_contract::TaintSource;
use scheduler_lite_contract::{Priority, Resource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::TaskId;

/// Klasa zadania = priorytet bazowy (PLAN §9.3: użytkownik > agentki > tło; mowa użytkownika
/// wyprzedza wszystko przez `scheduler-lite` — voice-first).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    /// Praca w tle (konsolidacja, wyzwalacze, porządki) — budżet tła z `cost-meter`.
    Background,
    /// Praca agentek z własnej inicjatywy albo delegowana.
    Agent,
    /// Zlecona bezpośrednio przez użytkownika.
    User,
}

impl TaskClass {
    /// Priorytet dzierżaw zasobów zadania tej klasy.
    pub fn lease_priority(self) -> Priority {
        match self {
            Self::Background => Priority::Background,
            Self::Agent => Priority::Normal,
            Self::User => Priority::Interactive,
        }
    }
}

/// Kto ma wykonać zadanie.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Assignee {
    /// Wskazana agentka.
    Persona(PersonaId),
    /// Dowolna dostępna agentka obsadzona w roli (uprawnienia idą za rolą, PLAN §9.2).
    Role(RoleId),
    /// Dowolna dostępna agentka.
    AnyAgent,
    /// Usługa systemowa bez persony i głosu (nie może trzymać głośnika).
    System(String),
}

/// Skąd pochodzi zadanie. Podzadania **dziedziczą** pochodzenie rodzica bez zmian — agentka nie
/// może „wyprać” wyzwalacza w żądanie użytkownika (PLAN §1.3 pkt 4, subscription-routes §2.4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum TaskOrigin {
    /// Bezpośrednie żądanie użytkownika (tekst, głos, paleta).
    User,
    /// Agentka z własnej inicjatywy.
    Agent {
        /// Agentka.
        persona: PersonaId,
    },
    /// Wyzwalacz (`triggers`); `depth` = długość łańcucha wyzwalaczy (ochrona przed pętlą).
    Trigger {
        /// Wyzwalacz.
        trigger_id: String,
        /// Głębokość łańcucha (1 = wyzwalacz uruchomiony zdarzeniem spoza wyzwalaczy).
        depth: u32,
    },
    /// Harmonogram jawnie dopuszczony przez użytkownika do mostów (`allow_bridges`).
    Schedule {
        /// Harmonogram.
        schedule_id: String,
    },
    /// Ulepszacz (§12) — mostów nigdy.
    Improver,
    /// Usługa systemowa.
    System {
        /// Usługa.
        service: String,
    },
}

impl TaskOrigin {
    /// Pochodzenie dla mostów CLI (`agent-backends`). Wszystko, co nie jest żądaniem użytkownika
    /// ani jawnie dopuszczonym harmonogramem, idzie jako uruchomienie automatyczne
    /// (`Trigger`) — most je odrzuca.
    pub fn launch_origin(&self) -> LaunchOrigin {
        match self {
            Self::User => LaunchOrigin::UserRequest,
            Self::Schedule { schedule_id } => LaunchOrigin::Scheduled {
                schedule_id: schedule_id.clone(),
            },
            Self::Improver => LaunchOrigin::Improver,
            Self::Trigger { trigger_id, .. } => LaunchOrigin::Trigger {
                trigger_id: trigger_id.clone(),
            },
            Self::Agent { persona } => LaunchOrigin::Trigger {
                trigger_id: format!("auto:agent:{persona}"),
            },
            Self::System { service } => LaunchOrigin::Trigger {
                trigger_id: format!("auto:system:{service}"),
            },
        }
    }

    /// Czy zadanie z tym pochodzeniem w ogóle może celować w most (`User` albo `Schedule`;
    /// zgodę i dzienny limit harmonogramu sprawdza dalej `agent-backends`).
    pub fn may_target_bridge(&self) -> bool {
        matches!(self, Self::User | Self::Schedule { .. })
    }

    /// Głębokość łańcucha wyzwalaczy (0 = nie z wyzwalacza).
    pub fn trigger_depth(&self) -> u32 {
        match self {
            Self::Trigger { depth, .. } => *depth,
            _ => 0,
        }
    }
}

/// Czym zadanie jest wykonywane.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ExecutorKind {
    /// Pętla agentki (`agent-runtime`).
    #[default]
    Agent,
    /// Most CLI (`agent-backends`) — tylko z pochodzenia `User` albo `Schedule`.
    Bridge(BridgeKind),
    /// Usługa systemowa (np. konsolidacja pamięci).
    Service(String),
}

/// Okno czasowe (czas ścienny, ms od epoki UTC).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimeWindow {
    /// Nie wcześniej niż.
    #[serde(default)]
    pub not_before_ms: Option<u64>,
    /// Termin: po nim zadanie kończy się `Expired` (domyślnie zgłoszenie + 24 h — każde
    /// zadanie kończy się w skończonym czasie).
    #[serde(default)]
    pub deadline_ms: Option<u64>,
    /// Tylko, gdy użytkownik jest bezczynny (praca w tle nie przeszkadza).
    #[serde(default)]
    pub only_when_idle: bool,
    /// Nie w trybie gry / pełnego ekranu.
    #[serde(default)]
    pub not_in_game_mode: bool,
}

/// Budżety zadania — łącznie dla wszystkich prób i wznowień.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskBudget {
    /// Najwięcej kroków atomowych.
    pub max_steps: u32,
    /// Najdłuższy łączny czas wykonania (ms, bez czekania w kolejce i pauz).
    pub max_wall_ms: u64,
    /// Najwyższy koszt (mikro-PLN); `None` = bez limitu zadania.
    #[serde(default)]
    pub max_cost_micro_pln: Option<u64>,
    /// Szacunek kosztu do decyzji budżetu tła (`cost-meter`); 0 = model lokalny.
    #[serde(default)]
    pub estimated_cost_micro_pln: u64,
}

impl Default for TaskBudget {
    fn default() -> Self {
        Self {
            max_steps: 40,
            max_wall_ms: 15 * 60 * 1000,
            max_cost_micro_pln: None,
            estimated_cost_micro_pln: 0,
        }
    }
}

/// Ponowienia z wykładniczym odstępem (deterministycznym — bez losowości).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RetryPolicy {
    /// Najwięcej prób (1 = bez ponowień).
    pub max_attempts: u32,
    /// Pierwszy odstęp (ms).
    pub initial_backoff_ms: u64,
    /// Najdłuższy odstęp (ms).
    pub max_backoff_ms: u64,
    /// Mnożnik kolejnych odstępów (≥ 1).
    pub multiplier: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff_ms: 1_000,
            max_backoff_ms: 60_000,
            multiplier: 2,
        }
    }
}

impl RetryPolicy {
    /// Bez ponowień.
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            ..Self::default()
        }
    }

    /// Odstęp po `failures`-tej porażce (1 = pierwsza): `initial · multiplier^(failures−1)`,
    /// najwyżej `max_backoff_ms`.
    pub fn backoff_ms(&self, failures: u32) -> u64 {
        let exp = failures.saturating_sub(1).min(32);
        let factor = u64::from(self.multiplier.max(1)).saturating_pow(exp);
        self.initial_backoff_ms
            .saturating_mul(factor)
            .min(self.max_backoff_ms)
    }
}

/// Warunek zależności (wyniki pośrednie i rozgałęzienia).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "when", rename_all = "snake_case")]
pub enum DepCondition {
    /// Poprzedniczka zakończona sukcesem (domyślnie).
    #[default]
    Succeeded,
    /// Poprzedniczka zakończona niepowodzeniem (błąd, budżet, termin) — gałąź naprawcza.
    Failed,
    /// Poprzedniczka zakończona w dowolny sposób.
    Finished,
    /// Sukces i wynik pośredni `key` równy `value`.
    OutputEquals {
        /// Klucz w `TaskOutput::values`.
        key: String,
        /// Oczekiwana wartość.
        value: serde_json::Value,
    },
}

/// Krawędź DAG: to zadanie czeka na `task`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Dependency {
    /// Poprzedniczka.
    pub task: TaskId,
    /// Warunek.
    #[serde(default)]
    pub condition: DepCondition,
}

impl Dependency {
    /// Zależność „po sukcesie”.
    pub fn on(task: impl Into<TaskId>) -> Self {
        Self {
            task: task.into(),
            condition: DepCondition::Succeeded,
        }
    }

    /// Zależność z warunkiem.
    pub fn when(task: impl Into<TaskId>, condition: DepCondition) -> Self {
        Self {
            task: task.into(),
            condition,
        }
    }
}

/// Specyfikacja zadania.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskSpec {
    /// Identyfikator (unikalny).
    pub id: TaskId,
    /// Tytuł dla Osi czasu (krótki; bez danych wrażliwych).
    pub title: String,
    /// Zadanie-rodzic (delegacja) — anulowanie rodzica anuluje poddrzewo.
    #[serde(default)]
    pub parent: Option<TaskId>,
    /// Zależności (DAG).
    #[serde(default)]
    pub deps: Vec<Dependency>,
    /// Przydział.
    pub assignee: Assignee,
    /// Klasa priorytetu.
    pub class: TaskClass,
    /// Pochodzenie.
    pub origin: TaskOrigin,
    /// Wykonawca.
    #[serde(default)]
    pub executor: ExecutorKind,
    /// Zasoby wyłączne (przyznawane razem albo wcale).
    #[serde(default)]
    pub resources: Vec<Resource>,
    /// Okno czasowe.
    #[serde(default)]
    pub window: TimeWindow,
    /// Budżety.
    #[serde(default)]
    pub budget: TaskBudget,
    /// Ponowienia.
    #[serde(default)]
    pub retry: RetryPolicy,
    /// Źródła niezaufanej treści (taint) — dziedziczone przez podzadania.
    #[serde(default)]
    pub taint: Vec<TaintSource>,
    /// Sesja, z której pochodzi zadanie.
    #[serde(default)]
    pub session: Option<SessionId>,
    /// Ładunek dla wykonawcy (cel, kontekst) — scheduler go nie interpretuje.
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl TaskSpec {
    /// Zadanie z wartościami domyślnymi.
    pub fn new(
        id: impl Into<TaskId>,
        title: impl Into<String>,
        assignee: Assignee,
        class: TaskClass,
        origin: TaskOrigin,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            parent: None,
            deps: Vec::new(),
            assignee,
            class,
            origin,
            executor: ExecutorKind::Agent,
            resources: Vec::new(),
            window: TimeWindow::default(),
            budget: TaskBudget::default(),
            retry: RetryPolicy::default(),
            taint: Vec::new(),
            session: None,
            payload: serde_json::Value::Null,
        }
    }

    /// Zależności (builder).
    #[must_use]
    pub fn after(mut self, deps: impl IntoIterator<Item = Dependency>) -> Self {
        self.deps.extend(deps);
        self
    }

    /// Zasoby wyłączne (builder).
    #[must_use]
    pub fn with_resources(mut self, resources: impl IntoIterator<Item = Resource>) -> Self {
        self.resources.extend(resources);
        self
    }

    /// Czy treść zadania jest niezaufana.
    pub fn is_tainted(&self) -> bool {
        !self.taint.is_empty()
    }
}
