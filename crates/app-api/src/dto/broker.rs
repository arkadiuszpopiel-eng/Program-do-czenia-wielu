//! Stan połączenia z Brokerem (komenda `broker_status`, zdarzenie `BrokerStatus`) — odpowiednik
//! `types-broker.ts`. UI pokazuje tryb (usługa / przenośny / w procesie), stan łącza i wyjaśnienie;
//! niczego tu się nie zatwierdza (zatwierdzenia wyłącznie w oknie Brokera, PLAN §8.2).

use serde::{Deserialize, Serialize};

use super::SecretInput;
use super::common::{AutonomyLevel, LocalizedText};

/// Gdzie działa Broker (ADR 0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrokerMode {
    /// Usługa Windows `AlfaBroker` na osobnym koncie (pełna izolacja, Broker-UI z wysoką
    /// integralnością).
    Service,
    /// Tryb przenośny: `alfa-broker --console` jako proces potomny aplikacji, na koncie
    /// użytkownika — słabsza izolacja (Audyt i okno Brokera bez ochrony osobnego konta i UIPI).
    Portable,
    /// Broker w procesie aplikacji (tryb deweloperski, Linux/CI) — bez okna zatwierdzeń.
    InProcess,
    /// Brak Brokera — nic, co wymaga zgody, nie zostanie wykonane.
    Unavailable,
}

/// Stan łącza z Brokerem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrokerLinkState {
    Connected,
    Connecting,
    /// Połączenie zerwane: bezpieczny stan — wszystko, co wymaga zgody, jest odrzucane.
    Lost,
}

/// Stan Brokera dla Ustawień → Uprawnienia i bezpieczeństwo oraz banera w oknie głównym.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BrokerStatusView {
    pub mode: BrokerMode,
    pub state: BrokerLinkState,
    /// Działa okno zatwierdzeń (Broker-UI) — karta „czeka na zatwierdzenie” może do niego przenieść.
    pub approval_window: bool,
    /// `Ctrl+Shift+F12` obsługuje `alfa-watchdog` poza UI (inaczej skrót rejestruje aplikacja).
    pub watchdog: bool,
    /// Broker na osobnym koncie (usługa) — tylko wtedy pełna izolacja z ADR 0003.
    pub isolated: bool,
    /// Wyjaśnienie trybu albo przyczyny zerwania (zwykły tekst).
    pub detail: Option<LocalizedText>,
}

impl BrokerStatusView {
    /// Broker w procesie aplikacji (z oknem zatwierdzeń albo bez).
    pub fn in_process(approval_window: bool) -> Self {
        let detail = if approval_window {
            LocalizedText::new(
                "Sędzia Cieni działa w procesie aplikacji (tryb deweloperski).",
                "The Shadow Judge runs inside the app process (developer mode).",
            )
        } else {
            LocalizedText::new(
                "Sędzia Cieni działa w procesie aplikacji bez okna zatwierdzeń (tryb deweloperski): \
                 każda prośba o zgodę jest odrzucana. To kopia Alfy bez programów Jądra obok \
                 (build testowy) — Alfę z oknem zgód uruchamia skrót „Alfa” na pulpicie albo \
                 w menu Start (folder Alfa-uruchomiona).",
                "The Shadow Judge runs inside the app process without the approval window (developer \
                 mode): every approval request is denied. This copy of Alfa has no Kernel \
                 programs next to it (a test build) — the “Alfa” shortcut on the desktop or in \
                 the Start menu (folder Alfa-uruchomiona) starts Alfa with the approval window.",
            )
        };
        Self {
            mode: BrokerMode::InProcess,
            state: BrokerLinkState::Connected,
            approval_window,
            watchdog: false,
            isolated: false,
            detail: Some(detail),
        }
    }

    /// Broker niepodłączony (budowa modułu nie powiodła się).
    pub fn unavailable() -> Self {
        Self {
            mode: BrokerMode::Unavailable,
            state: BrokerLinkState::Lost,
            approval_window: false,
            watchdog: false,
            isolated: false,
            detail: Some(LocalizedText::new(
                "Sędzia Cieni jest niedostępny — agentki nie wykonają niczego, co wymaga zgody.",
                "The Shadow Judge is unavailable — agents will not do anything that needs approval.",
            )),
        }
    }
}

/// Zgoda „zawsze w tym zakresie” w panelu uprawnień (komenda `permissions_grants`); odwołanie —
/// `permissions_revoke_grant` (zawsze bez okna Sędziego: tylko zawęża uprawnienia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GrantView {
    /// Numer zgody w Sędzim Cieni.
    pub id: String,
    pub session_id: String,
    /// Agentka (brak — cała sesja).
    pub agent: Option<String>,
    /// Zakres po ludzku („połączenie z internetem: strona x.example.org”).
    pub scope: String,
    pub granted_at_ms: u64,
    /// Do kiedy; `None` — do odwołania.
    pub until_ms: Option<u64>,
}

/// Uprawnienia (PLAN §8.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PermissionsState {
    pub global: AutonomyLevel,
    pub session: Option<AutonomyLevel>,
    pub hello_enabled: bool,
    /// Włączony „Max dostęp” (plan 50 §8.1); `None` — wyłączony albo Broker niepodłączony.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_access: Option<MaxAccess>,
}

/// Zasięg „Max dostęp” (plan 50 §8.1): bez pytań na modelach lokalnych, w chmurze albo wszystkich.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MaxAccess {
    Local,
    Cloud,
    All,
}

/// Stan intencji Brokera: `opened_broker` — prośba czeka w oknie Brokera; `applied` — zmiana
/// niewymagająca zatwierdzenia (obniżenie poziomu) została zastosowana od razu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrokerIntentStatus {
    OpenedBroker,
    Applied,
}

/// Wynik intencji Brokera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BrokerIntentResult {
    pub status: BrokerIntentStatus,
    pub request_id: String,
}

/// Profil Dziecko (plan 50 §4.4): stan kłódki trybów pracy (bez PIN-u).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct ChildLockState {
    pub pin_set: bool,
    pub enabled: bool,
    /// Kłódka otwarta teraz PIN-em rodzica.
    pub open: bool,
    /// Do kiedy otwarta (ms od 1970); `None` — zamknięta albo do zamknięcia Alfy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_until_ms: Option<u64>,
    /// Blokada po błędnych próbach do (ms od 1970).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_until_ms: Option<u64>,
    pub attempts_left: u32,
}

/// Zmiana profilu Dziecko; PIN wpisuje rodzic, sprawdza go wyłącznie Sędzia Cieni.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ChildLockRequest {
    /// Ustawienie PIN-u (zmiana wymaga starego).
    SetPin {
        old: Option<SecretInput>,
        new: SecretInput,
    },
    /// Włączenie albo wyłączenie profilu.
    Enable { pin: SecretInput, on: bool },
    /// Otwarcie kłódki na `minutes` minut; `None` — do zamknięcia Alfy.
    Open {
        pin: SecretInput,
        minutes: Option<u32>,
    },
    /// Zamknięcie kłódki (bez PIN-u).
    Close,
}
