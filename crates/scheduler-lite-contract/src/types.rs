//! Typy schedulera: zasoby wyłączne, posiadaczki, priorytety, polityki, żądania, błędy.

use std::fmt;
use std::time::Duration;

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Najdłuższe dozwolone `max_wait` (czekanie jest zawsze skończone).
pub const MAX_WAIT_LIMIT: Duration = Duration::from_secs(600);

/// Zasób wyłączny (PLAN §9.3). Jeden holder naraz.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "kind", content = "path", rename_all = "snake_case")]
pub enum Resource {
    /// Głośnik / mówienie — kolejka mówienia (jedna agentka naraz).
    Speaker,
    /// Mikrofon.
    Mic,
    /// Ekran + mysz/klawiatura (sterowanie GUI).
    ScreenInput,
    /// Wskazany plik (ścieżka znormalizowana: `/`, małe litery — Windows nie rozróżnia wielkości).
    File(String),
}

impl Resource {
    /// Zasób pliku ze znormalizowaną ścieżką.
    pub fn file(path: &str) -> Self {
        Self::File(path.trim().replace('\\', "/").to_lowercase())
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Speaker => f.write_str("speaker"),
            Self::Mic => f.write_str("mic"),
            Self::ScreenInput => f.write_str("screen_input"),
            Self::File(path) => write!(f, "file:{path}"),
        }
    }
}

/// Kto trzyma lub żąda zasobu.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Holder {
    /// Użytkownik (np. jego wypowiedź trzyma „głos” — voice-first).
    User,
    /// Agentka.
    Persona(PersonaId),
    /// Usługa systemowa (bez persony i głosu — nie może trzymać `Speaker`).
    System(String),
}

impl fmt::Display for Holder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => f.write_str("user"),
            Self::Persona(p) => write!(f, "persona:{p}"),
            Self::System(m) => write!(f, "system:{m}"),
        }
    }
}

/// Priorytet żądania (rosnąco). Wyższy wyprzedza w kolejce i może wywłaszczyć niższy.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// Praca w tle.
    Background,
    /// Narracja / fillery (wywłaszczane przez mowę użytkownika).
    Narration,
    /// Zwykła odpowiedź agentki.
    Normal,
    /// Interakcja wywołana przez użytkownika.
    Interactive,
    /// Mowa użytkownika (voice-first).
    UserSpeech,
    /// Krytyczne (np. komunikat bezpieczeństwa).
    Critical,
}

/// Co zrobić po przekroczeniu `max_wait`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OnTimeout {
    /// Zapytać użytkownika (UI dostaje zdarzenie `scheduler.lease.timeout`).
    AskUser,
    /// Zakończyć błędem.
    Fail,
}

/// Polityka zasobu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResourcePolicy {
    /// Posiadaczkę można wywłaszczyć, ale tylko w punkcie atomowym: dostaje sygnał
    /// `PreemptRequested` i sama zwalnia zasób (nie jest zabijana).
    pub preemptible_at_atomic: bool,
    /// Domyślne zachowanie po przekroczeniu czasu.
    pub on_timeout: OnTimeout,
    /// Jak długo zasób czeka na adresatkę przekazania (`handoff`), zanim wróci do kolejki (ms).
    pub handoff_reserve_ms: u64,
}

impl ResourcePolicy {
    /// Polityka domyślna (PLAN §9.4: `gui-exclusive` → `ask_user`, `voice-first` → wywłaszczanie narracji).
    pub fn default_for(resource: &Resource) -> Self {
        match resource {
            Resource::Speaker => Self {
                preemptible_at_atomic: true,
                on_timeout: OnTimeout::Fail,
                handoff_reserve_ms: 2_000,
            },
            Resource::ScreenInput => Self {
                preemptible_at_atomic: true,
                on_timeout: OnTimeout::AskUser,
                handoff_reserve_ms: 2_000,
            },
            Resource::Mic | Resource::File(_) => Self {
                preemptible_at_atomic: false,
                on_timeout: OnTimeout::Fail,
                handoff_reserve_ms: 2_000,
            },
        }
    }
}

/// Żądanie zasobu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LeaseRequest {
    /// Zasób.
    pub resource: Resource,
    /// Żądająca.
    pub holder: Holder,
    /// Priorytet.
    pub priority: Priority,
    /// Najdłuższe czekanie (0 = tylko natychmiastowe przyznanie); ≤ [`MAX_WAIT_LIMIT`].
    pub max_wait: Duration,
    /// Zachowanie po czasie; `None` = z polityki zasobu.
    #[serde(default)]
    pub on_timeout: Option<OnTimeout>,
}

impl LeaseRequest {
    /// Nowe żądanie z `on_timeout` z polityki zasobu.
    pub fn new(resource: Resource, holder: Holder, priority: Priority, max_wait: Duration) -> Self {
        Self {
            resource,
            holder,
            priority,
            max_wait,
            on_timeout: None,
        }
    }

    /// Nadpisuje zachowanie po czasie (builder).
    #[must_use]
    pub fn on_timeout(mut self, on_timeout: OnTimeout) -> Self {
        self.on_timeout = Some(on_timeout);
        self
    }
}

/// Identyfikator dzierżawy (lease).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct LeaseId(pub u64);

/// Identyfikator żądania (rośnie monotonicznie — „najmłodsze” = największe).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct RequestId(pub u64);

/// Opis przyznanej dzierżawy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LeaseInfo {
    /// Dzierżawa.
    pub id: LeaseId,
    /// Żądanie, z którego powstała.
    pub request: RequestId,
    /// Zasób.
    pub resource: Resource,
    /// Posiadaczka.
    pub holder: Holder,
    /// Priorytet.
    pub priority: Priority,
    /// Chwila przyznania (ms zegara schedulera).
    pub granted_at_ms: u64,
}

/// Żądanie czekające w kolejce (widok dla UI: kto czeka na głos).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueuedRequest {
    /// Żądanie.
    pub id: RequestId,
    /// Żądająca.
    pub holder: Holder,
    /// Priorytet.
    pub priority: Priority,
    /// Chwila zakolejkowania (ms).
    pub enqueued_at_ms: u64,
    /// Termin (ms).
    pub deadline_ms: u64,
}

/// Powód wywłaszczenia / odebrania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreemptReason {
    /// Mowa użytkownika (voice-first).
    UserSpeaks,
    /// Czeka żądanie o wyższym priorytecie.
    HigherPriority,
    /// Przekazanie („Przekazuję Delcie…”).
    Handoff,
    /// Kill-switch (odebranie wszystkiego).
    KillSwitch,
}

/// Sygnał dla posiadaczki dzierżawy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "signal", rename_all = "snake_case")]
pub enum LeaseSignal {
    /// Dzierżawa aktywna.
    Active,
    /// Proszę zwolnić w najbliższym punkcie atomowym.
    PreemptRequested {
        /// Kto czeka.
        by: Holder,
        /// Dlaczego.
        reason: PreemptReason,
    },
    /// Dzierżawa odebrana (kill-switch, przekazanie) — zasób już nie należy do posiadaczki.
    Revoked {
        /// Dlaczego.
        reason: PreemptReason,
    },
}

/// Błędy schedulera.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum SchedError {
    /// Przekroczono `max_wait`.
    #[error("zasób {resource} niedostępny po {waited_ms} ms ({on_timeout:?})")]
    Timeout {
        /// Zasób.
        resource: Resource,
        /// Zachowanie (czy pytać użytkownika).
        on_timeout: OnTimeout,
        /// Czas czekania.
        waited_ms: u64,
    },
    /// Wykryto zakleszczenie — błąd dostaje najmłodsze żądanie w cyklu.
    #[error("zakleszczenie przy {resource}: cykl {}", cycle.iter().map(ToString::to_string).collect::<Vec<_>>().join(" → "))]
    Deadlock {
        /// Zasób, na który czekało odrzucone żądanie.
        resource: Resource,
        /// Posiadaczki w cyklu.
        cycle: Vec<Holder>,
    },
    /// Żądanie anulowane (kill-switch).
    #[error("żądanie anulowane")]
    Cancelled,
    /// Posiadaczka już trzyma ten zasób.
    #[error("{holder} już trzyma {resource}")]
    AlreadyHeld {
        /// Zasób.
        resource: Resource,
        /// Posiadaczka.
        holder: Holder,
    },
    /// Usługi systemowe nie mówią (nie mają persony ani głosu).
    #[error("usługa systemowa nie może trzymać głośnika")]
    SystemCannotSpeak,
    /// `max_wait` ponad limit.
    #[error("max_wait {max_ms} ms przekracza limit")]
    InvalidMaxWait {
        /// Żądane `max_wait` (ms).
        max_ms: u64,
    },
    /// Zasób nie jest wywłaszczalny.
    #[error("zasobu {0} nie można wywłaszczyć")]
    NotPreemptible(Resource),
    /// Nikt nie trzyma zasobu.
    #[error("nikt nie trzyma {0}")]
    NotHeld(Resource),
    /// Nieznana (już zwolniona) dzierżawa.
    #[error("nieznana dzierżawa")]
    UnknownLease,
    /// Scheduler nie jest uruchomiony.
    #[error("scheduler nie jest uruchomiony")]
    NotStarted,
}

/// Milisekundy z `Duration` (nasycone).
pub fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}
