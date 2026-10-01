//! Porty modułów, których jeszcze nie ma w repo (router, transfer, głos, Broker) i powłoki
//! (okna, dialogi). Każdy ma domyślną implementację: czytelny błąd „funkcja dostępna po
//! podłączeniu modułu X" albo rozsądny zapas — kolejna sesja podpina moduł w jednym miejscu
//! (`AppOptions` → `compose.rs`).

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

pub use artifacts_contract::{ArtifactAction as ArtifactIntentAction, ArtifactIntent};
use async_trait::async_trait;
use providers_contract::ModelProvider;
use sessions_contract::{PrivacyTag, SessionId};

use crate::dto::{
    AudioDevice, AutonomyLevel, BrokerIntentResult, ExportRequest, ExportResult, ImportRequest,
    ImportResult, InspectResult, ModelProfile, SecretInput,
};
use crate::error::AppError;

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
}

/// Wybrany dostawca i model.
#[derive(Clone)]
pub struct BrainChoice {
    /// Dostawca (`ModelProvider`).
    pub provider: Arc<dyn ModelProvider>,
    /// Identyfikator dostawcy z katalogu (np. `anthropic`).
    pub provider_id: String,
    /// Nazwa dostawcy do UI.
    pub provider_name: String,
    /// Konto w `accounts-hub` (koszty per konto).
    pub account: Option<String>,
    /// Model.
    pub model: String,
    /// Okno kontekstu modelu, jeśli znane.
    pub context_window: Option<u64>,
}

/// Brak możliwości odpowiedzi.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BrainError {
    /// Brak kluczy/modelu („brak mózgu").
    #[error("{0}")]
    NoKeys(String),
    /// Dostawca skonfigurowany, ale niedostępny (np. nie da się wykryć modelu).
    #[error("{0}")]
    Provider(String),
}

/// „Mózg": wybór dostawcy i modelu. Docelowo moduł `router` (reguły, fallback, budżety).
#[async_trait]
pub trait BrainPort: Send + Sync {
    /// Wybiera dostawcę i model dla tury.
    async fn choose(&self, request: &BrainRequest) -> Result<BrainChoice, BrainError>;
    /// Czy jest choć jeden skonfigurowany dostawca czatu.
    fn keys_configured(&self) -> bool;
}

/// Import/eksport `.alfa` — docelowo moduł `transfer` (natywne dialogi po stronie powłoki).
#[async_trait]
pub trait TransferPort: Send + Sync {
    /// Eksport wg zakresu.
    async fn export(&self, request: ExportRequest) -> Result<ExportResult, AppError>;
    /// Eksport jednej sesji.
    async fn export_session(&self, session: &SessionId) -> Result<ExportResult, AppError>;
    /// Podgląd paczki (dry-run).
    async fn inspect(
        &self,
        password: Option<SecretInput>,
        path: Option<String>,
    ) -> Result<InspectResult, AppError>;
    /// Import.
    async fn import(&self, request: ImportRequest) -> Result<ImportResult, AppError>;
    /// Cofnięcie importu.
    async fn rollback(&self, snapshot: &str) -> Result<(), AppError>;
}

/// Domyślny port: moduł `transfer` niepodłączony.
pub struct TransferUnavailable;

const TRANSFER: &str = "transfer";

#[async_trait]
impl TransferPort for TransferUnavailable {
    async fn export(&self, _request: ExportRequest) -> Result<ExportResult, AppError> {
        Err(AppError::unavailable("Eksport paczki .alfa", TRANSFER))
    }
    async fn export_session(&self, _session: &SessionId) -> Result<ExportResult, AppError> {
        Err(AppError::unavailable("Eksport sesji do .alfa", TRANSFER))
    }
    async fn inspect(
        &self,
        _password: Option<SecretInput>,
        _path: Option<String>,
    ) -> Result<InspectResult, AppError> {
        Err(AppError::unavailable("Podgląd paczki .alfa", TRANSFER))
    }
    async fn import(&self, _request: ImportRequest) -> Result<ImportResult, AppError> {
        Err(AppError::unavailable("Import paczki .alfa", TRANSFER))
    }
    async fn rollback(&self, _snapshot: &str) -> Result<(), AppError> {
        Err(AppError::unavailable("Cofnięcie importu", TRANSFER))
    }
}

/// Głos — docelowo moduły `voice-*` (audio, STT/TTS, pigułka).
#[async_trait]
pub trait VoicePort: Send + Sync {
    /// Urządzenia wejściowe; `None` = użyj listy z `device-profile`.
    async fn devices(&self) -> Result<Option<Vec<AudioDevice>>, AppError>;
    /// Start testu mikrofonu (poziomy przez zdarzenie `MicLevel`).
    async fn start_mic_test(&self, device: Option<String>) -> Result<(), AppError>;
    /// Stop testu mikrofonu.
    async fn stop_mic_test(&self) -> Result<(), AppError>;
    /// Mikrofon wł./wył.
    async fn set_mic_enabled(&self, enabled: bool) -> Result<(), AppError>;
    /// Wyciszenie.
    async fn set_muted(&self, muted: bool) -> Result<(), AppError>;
    /// Stop mowy.
    async fn stop_speech(&self) -> Result<(), AppError>;
    /// Czytanie tekstu głosem agentki.
    async fn read_aloud(&self, agent: &str, text: &str) -> Result<(), AppError>;
}

/// Domyślny port: głos niepodłączony. Operacje „wyłączające" (stop, wycisz, mikrofon wył.)
/// są bezpiecznym no-op, „włączające" zwracają błąd z nazwą modułu.
pub struct VoiceUnavailable;

#[async_trait]
impl VoicePort for VoiceUnavailable {
    async fn devices(&self) -> Result<Option<Vec<AudioDevice>>, AppError> {
        Ok(None)
    }
    async fn start_mic_test(&self, _device: Option<String>) -> Result<(), AppError> {
        Err(AppError::unavailable("Test mikrofonu", "voice-audio"))
    }
    async fn stop_mic_test(&self) -> Result<(), AppError> {
        Ok(())
    }
    async fn set_mic_enabled(&self, enabled: bool) -> Result<(), AppError> {
        if enabled {
            return Err(AppError::unavailable("Mikrofon", "voice-audio"));
        }
        Ok(())
    }
    async fn set_muted(&self, _muted: bool) -> Result<(), AppError> {
        Ok(())
    }
    async fn stop_speech(&self) -> Result<(), AppError> {
        Ok(())
    }
    async fn read_aloud(&self, _agent: &str, _text: &str) -> Result<(), AppError> {
        Err(AppError::unavailable("Czytanie na głos", "voice-tts"))
    }
}

/// Broker (okno zatwierdzeń, poziomy autonomii, uruchamianie kodu) — docelowo `safety-broker`.
#[async_trait]
pub trait BrokerPort: Send + Sync {
    /// Prośba o zmianę poziomu autonomii (potwierdzenie tylko w oknie Brokera).
    async fn request_level(
        &self,
        level: AutonomyLevel,
        session: Option<&SessionId>,
    ) -> Result<BrokerIntentResult, AppError>;
    /// Przeniesienie do karty zatwierdzenia w oknie Brokera.
    async fn open_approval(&self, approval: &str) -> Result<BrokerIntentResult, AppError>;
    /// Uruchomienie bloku kodu w terminalu (zawsze przez Brokera).
    async fn run_code(
        &self,
        session: &SessionId,
        lang: Option<&str>,
        code: &str,
    ) -> Result<BrokerIntentResult, AppError>;
    /// Cofnięcie kroku narzędzia (dziennik cofania `fs.*`).
    async fn undo_step(&self, token: &str) -> Result<(), AppError>;
}

/// Domyślny port: Broker niepodłączony (F3).
pub struct BrokerUnavailable;

#[async_trait]
impl BrokerPort for BrokerUnavailable {
    async fn request_level(
        &self,
        _level: AutonomyLevel,
        _session: Option<&SessionId>,
    ) -> Result<BrokerIntentResult, AppError> {
        Err(AppError::unavailable(
            "Zmiana poziomu autonomii",
            "safety-broker",
        ))
    }
    async fn open_approval(&self, _approval: &str) -> Result<BrokerIntentResult, AppError> {
        Err(AppError::unavailable("Okno zatwierdzeń", "safety-broker"))
    }
    async fn run_code(
        &self,
        _session: &SessionId,
        _lang: Option<&str>,
        _code: &str,
    ) -> Result<BrokerIntentResult, AppError> {
        Err(AppError::unavailable(
            "Uruchomienie kodu w terminalu",
            "safety-broker",
        ))
    }
    async fn undo_step(&self, _token: &str) -> Result<(), AppError> {
        Err(AppError::unavailable("Cofnięcie kroku", "undo-journal"))
    }
}

/// Powłoka (Tauri): okna, natywne dialogi i akcje systemowe wykonywane jako użytkownik.
pub trait ShellPort: Send + Sync {
    /// Pokazuje okno główne (opcjonalnie z sesją).
    fn show_main(&self, session: Option<&str>) -> Result<(), AppError>;
    /// Chowa okno Szybkiego pytania.
    fn hide_quick(&self) -> Result<(), AppError>;
    /// Otwiera stronę ustawień Windows (URI już sprawdzony z listą dozwolonych).
    fn open_system_settings(&self, uri: &str) -> Result<(), AppError>;
    /// Wykonuje zwalidowaną intencję pliku (Otwórz, Pokaż w Eksploratorze, Kopiuj, Zapisz jako).
    fn artifact_action(&self, intent: &ArtifactIntent) -> Result<(), AppError>;
    /// Natywny dialog „Zapisz jako" dla tekstu; `false` = anulowano.
    fn save_text_as(&self, suggested_name: &str, text: &str) -> Result<bool, AppError>;
    /// Wolne miejsce na dysku z `path` (`None` = nieznane).
    fn disk_free(&self, _path: &Path) -> Option<u64> {
        None
    }
}

/// Powłoka bez okien (testy, tryb bezgłowy): zapisuje wywołania; dialogi niedostępne.
#[derive(Default)]
pub struct HeadlessShell {
    calls: Mutex<Vec<String>>,
}

impl HeadlessShell {
    /// Zarejestrowane wywołania (do asercji w testach).
    pub fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(call);
    }
}

impl ShellPort for HeadlessShell {
    fn show_main(&self, session: Option<&str>) -> Result<(), AppError> {
        self.record(format!("show_main:{}", session.unwrap_or("-")));
        Ok(())
    }
    fn hide_quick(&self) -> Result<(), AppError> {
        self.record("hide_quick".into());
        Ok(())
    }
    fn open_system_settings(&self, uri: &str) -> Result<(), AppError> {
        self.record(format!("open_system_settings:{uri}"));
        Ok(())
    }
    fn artifact_action(&self, intent: &ArtifactIntent) -> Result<(), AppError> {
        self.record(format!("artifact:{}:{:?}", intent.artifact, intent.action));
        Ok(())
    }
    fn save_text_as(&self, _suggested_name: &str, _text: &str) -> Result<bool, AppError> {
        Err(AppError::unavailable(
            "Zapis przez okno dialogowe",
            "shell-integration",
        ))
    }
}
