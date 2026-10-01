//! Żądania akcji, plany, decyzje i błędy Brokera.

use core_bus_contract::SessionId;
use risk_classifier_contract::{
    CommandOrigin, Destructiveness, KernelRule, Reversibility, RiskLevel, RuleId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::approval::ApprovalId;
use crate::autonomy::AutonomyTarget;
use crate::capability::Capability;
use crate::token::{CapToken, Holder, TokenId};

/// Fakty deklarowane przez narzędzie (manifest + argumenty). Broker dolicza resztę sam:
/// zakres, taint, reguły Jądra, egress-allowlistę.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct DeclaredFacts {
    /// Narzędzie (np. `tools-fs.delete`).
    pub tool: String,
    /// `reversible: yes|scoped|no` z manifestu narzędzia.
    pub reversible: Reversibility,
    /// Destrukcyjność.
    pub destructive: Destructiveness,
    /// Liczba obiektów.
    pub bulk: u32,
    /// Instalacja oprogramowania.
    pub install: bool,
    /// Argumenty pochodzą z niezaufanej treści.
    pub untrusted_input_in_args: bool,
    /// Akcja dotyka danych prywatnych (poza tym, co Broker wie sam).
    pub touches_private_data: bool,
    /// Pełne polecenie powłoki (dla `shell.exec` — sprawdzane regułami Jądra).
    pub command: Option<String>,
}

impl DeclaredFacts {
    /// Fakty minimalne: odwracalna, niedestrukcyjna, pojedyncza.
    pub fn new(tool: &str) -> Self {
        Self {
            tool: tool.to_owned(),
            reversible: Reversibility::Yes,
            destructive: Destructiveness::None,
            bulk: 1,
            install: false,
            untrusted_input_in_args: false,
            touches_private_data: false,
            command: None,
        }
    }
}

/// Żądanie tokenu dla akcji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ActionRequest {
    /// Kto prosi.
    pub holder: Holder,
    /// Potrzebna zdolność.
    pub capability: Capability,
    /// Fakty z narzędzia.
    pub facts: DeclaredFacts,
    /// Źródło polecenia.
    pub origin: CommandOrigin,
    /// Żądany TTL (ms); `None` = domyślny polityki; zawsze przycinany do maksimum.
    pub ttl_ms: Option<u64>,
}

/// Krok planu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanStep {
    /// Zdolność kroku (zakres pokrywa wiele akcji, np. 14 przeniesień w katalogu).
    pub capability: Capability,
    /// Fakty z narzędzia.
    pub facts: DeclaredFacts,
    /// Opis (zwykły tekst).
    pub description: String,
}

/// „Plan do zatwierdzenia” (§14.6): wiele akcji jednym zatwierdzeniem z zakresem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanRequest {
    /// Kto prosi.
    pub holder: Holder,
    /// Tytuł.
    pub title: String,
    /// Źródło polecenia (wspólne dla planu).
    pub origin: CommandOrigin,
    /// Kroki.
    pub steps: Vec<PlanStep>,
    /// Czas ważności planu po zatwierdzeniu (ms; przycinany do polityki).
    pub ttl_ms: u64,
}

/// Bilet prośby o zatwierdzenie (dla proszącego — bez nonce).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalTicket {
    /// Identyfikator (do `approval_status`).
    pub id: ApprovalId,
    /// Klasa ryzyka.
    pub risk: RiskLevel,
    /// Reguły, które zadziałały.
    pub rules: Vec<RuleId>,
    /// Wymaga potwierdzenia nie-głosem.
    pub non_voice: bool,
    /// Wyjaśnienie (po polsku).
    pub explanation: String,
}

/// Powód odmowy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "reason", content = "detail", rename_all = "snake_case")]
pub enum DenyReason {
    /// Twarda blokada Jądra (każdy poziom, także L4).
    KernelBlock(KernelRule),
    /// Audyt niedostępny — Broker nie wydaje tokenów bez zapisu decyzji (fail-closed).
    AuditUnavailable,
}

/// Decyzja o akcji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", content = "value", rename_all = "snake_case")]
pub enum Decision {
    /// Zezwolono — token z TTL.
    Allow(CapToken),
    /// Potrzebne zatwierdzenie w Broker-UI.
    NeedsApproval(ApprovalTicket),
    /// Odmowa.
    Deny(DenyReason),
}

/// Wynik złożenia planu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "plan", content = "value", rename_all = "snake_case")]
pub enum PlanDecision {
    /// Żaden krok nie wymaga zgody — plan aktywny od razu.
    Approved,
    /// Kroki wymagające zgody czekają na jedno zatwierdzenie.
    NeedsApproval(ApprovalTicket),
    /// Któryś krok narusza regułę Jądra — cały plan odrzucony.
    Rejected {
        /// Indeks kroku.
        step: usize,
        /// Reguła.
        rule: KernelRule,
    },
}

/// Żądanie tokenu potomnego (atenuacja).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttenuateRequest {
    /// Zdolność potomka (musi być podzbiorem rodzica).
    pub capability: Capability,
    /// Rola potomka (sesja i agentka jak u rodzica).
    pub role: Option<String>,
    /// TTL potomka (ms; przycinany do czasu życia rodzica).
    pub ttl_ms: u64,
}

/// Skąd przyszło żądanie zmiany poziomu/polityki (ustala serwer IPC z roli klienta).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "via", content = "id", rename_all = "snake_case")]
pub enum ChangeOrigin {
    /// Ustawienia/pasek sesji w UI (przekaźnik jądra).
    UserInterface,
    /// Polecenie głosowe właściciela („Delta, pracuj na Maksie”).
    UserVoice,
    /// Agentka — nie może podnieść poziomu ani zmienić polityk.
    Agent(String),
}

/// Żądanie zmiany poziomu autonomii.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AutonomyChangeRequest {
    /// Cel.
    pub target: AutonomyTarget,
    /// Poziom.
    pub level: risk_classifier_contract::AutonomyLevel,
    /// Do kiedy (ms), jeśli „na czas”.
    pub until_ms: Option<u64>,
    /// Źródło.
    pub origin: ChangeOrigin,
}

/// Źródło niezaufanej treści (taint sesji).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaintSource {
    /// Strona WWW.
    Web,
    /// E-mail.
    Email,
    /// Plik z zewnątrz.
    File,
    /// Treść ekranu (OCR/UIA).
    Screen,
    /// Dźwięk spoza właściciela (TV, rozmowa obok).
    Audio,
    /// Wynik/opis narzędzia MCP.
    Mcp,
}

/// Stan bezpieczeństwa sesji (monotoniczny: taint zdejmuje tylko nowa sesja).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionSecurity {
    /// Sesja widziała niezaufaną treść.
    pub tainted: bool,
    /// Źródła taintu.
    pub taint_sources: Vec<TaintSource>,
    /// Przebieg miał dostęp do danych prywatnych (składnik A trifecty).
    pub private_data: bool,
}

/// Metryki (Ustawienia → Uprawnienia: „pytania/godz.”).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BrokerMetrics {
    /// Prośby o zatwierdzenie w ostatniej godzinie.
    pub approvals_last_hour: u32,
    /// Aktywne tokeny.
    pub active_tokens: u32,
    /// Oczekujące prośby.
    pub pending_approvals: u32,
    /// Twarde blokady od startu.
    pub kernel_blocks: u64,
}

/// Błędy Brokera.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum BrokerError {
    /// Żądanie niepoprawne (pusta sesja, zły TTL, zakres).
    #[error("niepoprawne żądanie: {0}")]
    InvalidRequest(String),
    /// Token odrzucony (MAC, uruchomienie, podmiot, zakres, cofnięty).
    #[error("token odrzucony: {0}")]
    TokenRejected(String),
    /// Token wygasł.
    #[error("token wygasł")]
    TokenExpired,
    /// Potomek szerszy niż rodzic.
    #[error("atenuacja odrzucona: potomek nie może być szerszy niż rodzic")]
    NotAttenuated,
    /// Twarda blokada Jądra.
    #[error("blokada Jądra: {0:?}")]
    KernelBlock(KernelRule),
    /// Dowód fizycznego wejścia odrzucony.
    #[error("dowód fizycznego wejścia odrzucony: {0}")]
    ProofRejected(String),
    /// Nieznana prośba.
    #[error("nieznana prośba {0:?}")]
    UnknownApproval(ApprovalId),
    /// Nieznany token.
    #[error("nieznany token {0:?}")]
    UnknownToken(TokenId),
    /// Brak uprawnień do operacji (rola klienta IPC).
    #[error("brak uprawnień: {0}")]
    Unauthorized(String),
    /// Audyt niedostępny (fail-closed).
    #[error("audyt niedostępny: {0}")]
    AuditUnavailable(String),
    /// Nieznana sesja.
    #[error("nieznana sesja {0}")]
    UnknownSession(SessionId),
}
