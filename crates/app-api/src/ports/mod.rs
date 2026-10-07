//! Porty modułów i powłoki (okna, dialogi). Każdy ma implementację domyślną: moduł podpięty
//! w `app-core` (Router, transfer, Broker w procesie, głos) albo czytelny błąd „funkcja dostępna
//! po podłączeniu modułu X" — podmiana w jednym miejscu (`AppOptions` → `parts`).

mod modules;
mod shell;
mod voice;

use std::sync::Arc;

pub use artifacts_contract::{ArtifactAction as ArtifactIntentAction, ArtifactIntent};
use async_trait::async_trait;
use providers_contract::{ChatRequest, ModelProvider};
use sessions_contract::{PrivacyTag, SessionId};

pub use modules::{
    ApprovalWindow, AutonomyView, BrokerPort, BrokerUnavailable, KillOrigin, NEEDS_BROKER_WINDOW,
    NoApprovalWindow, TransferPort, TransferUnavailable,
};
pub use shell::{HeadlessShell, ShellPort};
pub use voice::{
    VOICE_PREVIEW_TEXT, VoiceChat, VoiceChunk, VoicePort, VoiceTurn, VoiceTurnOrigin, VoiceTurnRef,
    VoiceUnavailable, voice_unavailable_reason,
};

use crate::dto::ModelProfile;

/// Zapytanie o model dla tury (wejście routera, PLAN §5.4).
#[derive(Debug, Clone)]
pub struct BrainRequest {
    /// Sesja.
    pub session: SessionId,
    /// Agentka odpowiadająca.
    pub agent: String,
    /// Profil wybrany w UI (`None` = domyślny sesji/ustawień).
    pub profile: Option<ModelProfile>,
    /// Tag prywatności sesji (egzekwuje router; adapter — obrona w głąb).
    pub privacy: PrivacyTag,
    /// Żądanie (historia, narzędzia) — oszacowanie kosztu i wymagań przy wyborze trasy.
    pub chat: Option<ChatRequest>,
}

/// Uzasadnienie wyboru trasy (oś czasu, „dlaczego ten model"), bez treści rozmowy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RouteNote {
    /// Wybrany cel `dostawca:model`.
    pub chosen: String,
    /// Kolejne cele (fallback).
    pub fallbacks: Vec<String>,
    /// Odrzuceni kandydaci: „`dostawca:model` — powód".
    pub rejected: Vec<String>,
}

/// Wybrany dostawca i model.
#[derive(Clone)]
pub struct BrainChoice {
    /// Dostawca (`ModelProvider`; przy Routerze — dekorator z fallbackiem).
    pub provider: Arc<dyn ModelProvider>,
    /// Identyfikator dostawcy z katalogu (np. `anthropic`; Router — `router`).
    pub provider_id: String,
    /// Nazwa dostawcy do UI.
    pub provider_name: String,
    /// Konto w `accounts-hub` (koszty per konto).
    pub account: Option<String>,
    /// Model (Router: `auto` albo przypięty `dostawca:model`).
    pub model: String,
    /// Okno kontekstu modelu, jeśli znane.
    pub context_window: Option<u64>,
    /// Dostawca jest Routerem: `Started.model` to `dostawca:model` (cel rozpoznaje
    /// [`BrainPort::target`]), a budżet sprawdził Router per kandydat.
    pub routed: bool,
    /// Uzasadnienie wyboru trasy.
    pub route: Option<RouteNote>,
}

/// Cel trasy rozpoznany z `dostawca:model` (który model faktycznie odpowiada).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainTarget {
    /// Identyfikator dostawcy (np. `anthropic`, `local`).
    pub provider_id: String,
    /// Nazwa do UI.
    pub provider_name: String,
    /// Konto w `accounts-hub` (trasy API).
    pub account: Option<String>,
    /// Model u dostawcy.
    pub model: String,
    /// Model lokalny (bez ruchu sieciowego).
    pub local: bool,
}

/// Brak możliwości odpowiedzi.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BrainError {
    /// Brak kluczy/modelu („brak mózgu") — router nie ma żadnego kandydata.
    #[error("{0}")]
    NoKeys(String),
    /// Dostawca skonfigurowany, ale niedostępny albo trasa niedozwolona (prywatność, zgodność).
    #[error("{0}")]
    Provider(String),
    /// Wszystkie trasy odcięte limitem kosztów.
    #[error("{0}")]
    Budget(String),
}

/// „Mózg": wybór dostawcy i modelu (domyślnie Router z dostawcami z `accounts-hub`
/// i modelem lokalnym).
#[async_trait]
pub trait BrainPort: Send + Sync {
    /// Wybiera dostawcę i model dla tury.
    async fn choose(&self, request: &BrainRequest) -> Result<BrainChoice, BrainError>;
    /// Czy jest choć jeden skonfigurowany dostawca czatu przez API (bez kluczy — profil lokalny).
    fn keys_configured(&self) -> bool;
    /// Cel trasy z `dostawca:model` zwróconego w `Started` (tylko gdy `BrainChoice::routed`).
    fn target(&self, _model: &str) -> Option<BrainTarget> {
        None
    }
}
