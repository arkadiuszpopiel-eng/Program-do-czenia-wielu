//! Traity i typy cyklu Broker-UI: prośba z Brokera → karta w oknie → decyzja z dowodem
//! fizycznego wejścia → odpowiedź do Brokera.

use safety_broker_contract::ipc::ProofWire;
use safety_broker_contract::{ApprovalChallenge, ApprovalDecision, ApprovalId, PhysicalInputProof};
use serde::{Deserialize, Serialize};

use crate::guard::RejectReason;

/// Zdarzenie: karta pokazana.
pub const EVENT_SHOWN: &str = "broker_ui.shown";
/// Zdarzenie (Audyt przez Brokera): decyzja przekazana.
pub const EVENT_DECIDED: &str = "broker_ui.decided";
/// Zdarzenie: odrzucono wejście wstrzyknięte (próba SendInput/UIA).
pub const EVENT_INJECTION_REJECTED: &str = "broker_ui.injection_rejected";
/// Zdarzenie: odrzucono wejście z innego powodu (za wcześnie, zasłonięte, nieaktywne okno).
pub const EVENT_INPUT_REJECTED: &str = "broker_ui.input_rejected";
/// Zdarzenie: użyto Windows Hello.
pub const EVENT_HELLO_USED: &str = "broker_ui.hello.used";
/// Zdarzenie: karta wycofana (wygasła, rozstrzygnięta gdzie indziej, kill-switch).
pub const EVENT_WITHDRAWN: &str = "broker_ui.withdrawn";

/// Decyzja właściciela z dowodem (zużywana raz — dowód nie jest `Clone`).
#[derive(Debug, PartialEq, Eq)]
pub struct UiDecision {
    /// Prośba.
    pub id: ApprovalId,
    /// Decyzja.
    pub decision: ApprovalDecision,
    /// Dowód fizycznego wejścia.
    pub proof: PhysicalInputProof,
}

impl UiDecision {
    /// Postać przewodowa dowodu (żądanie `Resolve` przez IPC).
    pub fn proof_wire(&self) -> ProofWire {
        ProofWire {
            approval: self.proof.approval(),
            nonce: self.proof.nonce(),
            source: self.proof.source(),
            injected: self.proof.injected(),
            at_ms: self.proof.at_ms(),
        }
    }
}

/// Stan okna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UiStatus {
    /// Brak próśb — okno ukryte.
    Hidden,
    /// Oczekujące karty (liczba, nasycana do 255).
    Pending {
        /// Liczba.
        count: u8,
    },
    /// Zablokowane (np. brak połączenia z Brokerem).
    Blocked {
        /// Powód.
        reason: String,
    },
}

/// Zdarzenie diagnostyczne Broker-UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum UiEvent {
    /// [`EVENT_SHOWN`].
    Shown {
        /// Prośba.
        id: ApprovalId,
    },
    /// [`EVENT_DECIDED`].
    Decided {
        /// Prośba.
        id: ApprovalId,
        /// Decyzja.
        decision: ApprovalDecision,
    },
    /// [`EVENT_INJECTION_REJECTED`] albo [`EVENT_INPUT_REJECTED`].
    InputRejected {
        /// Prośba.
        id: ApprovalId,
        /// Powód.
        reason: RejectReason,
    },
    /// [`EVENT_HELLO_USED`].
    HelloUsed {
        /// Prośba.
        id: ApprovalId,
        /// Czy potwierdzono.
        verified: bool,
    },
    /// [`EVENT_WITHDRAWN`].
    Withdrawn {
        /// Prośba.
        id: ApprovalId,
    },
}

impl UiEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Shown { .. } => EVENT_SHOWN,
            Self::Withdrawn { .. } => EVENT_WITHDRAWN,
            Self::Decided { .. } => EVENT_DECIDED,
            Self::InputRejected {
                reason: RejectReason::Injected,
                ..
            } => EVENT_INJECTION_REJECTED,
            Self::InputRejected { .. } => EVENT_INPUT_REJECTED,
            Self::HelloUsed { .. } => EVENT_HELLO_USED,
        }
    }
}

/// Błąd Broker-UI.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UiError {
    /// Wyzwanie niepoprawne.
    #[error("niepoprawne wyzwanie: {0}")]
    InvalidChallenge(String),
    /// Błąd okna.
    #[error("okno Brokera: {0}")]
    Surface(String),
    /// Błąd połączenia z Brokerem (stan prośby nieznany — karta nie znika, przegląd Q-9).
    #[error("połączenie z Brokerem: {0}")]
    Link(String),
    /// Broker jawnie odpowiedział błędem na decyzję (prośba wygasła, rozstrzygnięta, dowód
    /// odrzucony, Audyt niedostępny) — przy odmowie prośba po stronie Brokera już nie czeka.
    #[error("Broker odrzucił decyzję: {0}")]
    Rejected(String),
}

/// Broker-UI: kolejka kart i decyzje z dowodem. Implementacje: natywne okno (`-impl`),
/// skrypt (`-fake`, tylko testy).
pub trait BrokerUi: Send {
    /// Przyjmuje wyzwanie (duplikat tej samej prośby jest ignorowany).
    fn show(&mut self, challenge: ApprovalChallenge, now_ms: u64) -> Result<(), UiError>;

    /// Obsługuje wejście (czeka do `wait_ms`) i zwraca decyzję, jeśli zapadła.
    fn poll_decision(&mut self, now_ms: u64, wait_ms: u32) -> Option<UiDecision>;

    /// Wycofuje kartę (prośba już nie oczekuje).
    fn withdraw(&mut self, id: ApprovalId);

    /// Identyfikatory kart w kolejce (w tym bieżącej).
    fn queued(&self) -> Vec<ApprovalId>;

    /// Stan okna.
    fn status(&self) -> UiStatus;

    /// Zdarzenia od ostatniego wywołania.
    fn drain_events(&mut self) -> Vec<UiEvent>;
}

/// Połączenie z Brokerem (kanał zatwierdzeń: tylko rola `BrokerUi`).
pub trait BrokerLink: Send {
    /// Oczekujące wyzwania.
    fn pending(&mut self) -> Result<Vec<ApprovalChallenge>, UiError>;

    /// Rozstrzygnięcie prośby (dowód zużywany).
    fn resolve(&mut self, decision: UiDecision) -> Result<(), UiError>;
}

/// Wynik Windows Hello.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelloOutcome {
    /// Tożsamość potwierdzona.
    Verified,
    /// Anulowane albo nieudane.
    Cancelled,
    /// Brak czujnika/PIN-u albo funkcja wyłączona.
    Unavailable,
}

/// Windows Hello (`UserConsentVerifier`) — w F3 typ i atrapa; implementacja Windows w SPEC v1.
pub trait HelloPort: Send + Sync {
    /// Prosi o potwierdzenie tożsamości (blokuje do wyniku).
    fn verify(&self, prompt: &str) -> HelloOutcome;
}

/// Brak Windows Hello (domyślnie; `hello_enabled = false`).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHello;

impl HelloPort for NoHello {
    fn verify(&self, _prompt: &str) -> HelloOutcome {
        HelloOutcome::Unavailable
    }
}

/// Konfiguracja `[broker_ui]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiConfig {
    /// Czas „zawsze w tym zakresie” w godzinach (1–24).
    pub grant_hours: u8,
    /// Windows Hello włączone.
    pub hello_enabled: bool,
    /// Co ile ms pytać Brokera o prośby.
    pub poll_ms: u64,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            grant_hours: 8,
            hello_enabled: false,
            poll_ms: 250,
        }
    }
}

impl UiConfig {
    /// Czas „zawsze w tym zakresie” w ms (przycięty do 1–24 h).
    pub fn grant_ms(&self) -> u64 {
        u64::from(self.grant_hours.clamp(1, 24)) * 60 * 60 * 1000
    }
}
