//! Porty modułów podpinanych w `app-core`: transfer, głos, Broker (z dziennikiem cofania) i okno
//! zatwierdzeń Brokera. Implementacje „niepodłączone" zwracają czytelny błąd z nazwą modułu.

use async_trait::async_trait;
use sessions_contract::SessionId;

use crate::dto::{
    AudioDevice, AutonomyLevel, BrokerIntentResult, ExportRequest, ExportResult, ImportRequest,
    ImportResult, InspectResult, SecretInput,
};
use crate::error::AppError;

/// Import/eksport `.alfa` — moduł `transfer` (natywne dialogi po stronie powłoki).
#[async_trait]
pub trait TransferPort: Send + Sync {
    /// Eksport wg zakresu.
    async fn export(&self, request: ExportRequest) -> Result<ExportResult, AppError>;
    /// Eksport jednej sesji.
    async fn export_session(&self, session: &SessionId) -> Result<ExportResult, AppError>;
    /// Jawny eksport sekretów — zawsze szyfrowany hasłem (PLAN §15.1).
    async fn export_secrets(&self, _password: SecretInput) -> Result<ExportResult, AppError> {
        Err(AppError::unavailable("Eksport sekretów", TRANSFER))
    }
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

/// Port: moduł `transfer` niepodłączony.
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

/// Głos — moduły `voice-*` (audio, TTS; potok rozmowy głosowej — `voice-pipeline`).
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

/// Port: głos niepodłączony. Operacje „wyłączające" (stop, wycisz, mikrofon wył.) są bezpiecznym
/// no-op, „włączające" zwracają błąd z nazwą modułu.
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
            return Err(AppError::unavailable("Mikrofon", "voice-pipeline"));
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

/// Poziomy autonomii obowiązujące teraz (z Brokera).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutonomyView {
    /// Poziom globalny.
    pub global: AutonomyLevel,
    /// Poziom sesji, gdy różni się od globalnego.
    pub session: Option<AutonomyLevel>,
}

/// Skąd przyszło „STOP WSZYSTKIEGO".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillOrigin {
    /// `Ctrl+Shift+F12`.
    Hotkey,
    /// Menu zasobnika.
    Tray,
    /// Przycisk w oknie (kapsuła aktywności).
    Ui,
}

/// Broker (poziomy autonomii, decyzje o akcjach, kill-switch) i dziennik cofania `fs.*`.
#[async_trait]
pub trait BrokerPort: Send + Sync {
    /// Poziomy z Brokera; `None` = Broker niepodłączony (rdzeń pokazuje poziom z konfiguracji).
    async fn levels(&self, _session: Option<&SessionId>) -> Option<AutonomyView> {
        None
    }
    /// Prośba o zmianę poziomu autonomii (podniesienie — tylko przez okno Brokera).
    async fn request_level(
        &self,
        level: AutonomyLevel,
        session: Option<&SessionId>,
    ) -> Result<BrokerIntentResult, AppError>;
    /// Przeniesienie do karty zatwierdzenia w oknie Brokera.
    async fn open_approval(&self, approval: &str) -> Result<BrokerIntentResult, AppError>;
    /// Uruchomienie bloku kodu w terminalu (zawsze decyzja Brokera).
    async fn run_code(
        &self,
        session: &SessionId,
        lang: Option<&str>,
        code: &str,
    ) -> Result<BrokerIntentResult, AppError>;
    /// Cofnięcie kroku narzędzia (dziennik cofania `fs.*`); zwraca opis cofniętego kroku.
    async fn undo_step(&self, session: &SessionId, step: u64) -> Result<String, AppError>;
    /// Kill-switch Brokera: unieważnia tokeny, zabija drzewa procesów, wycisza audio.
    async fn kill_all(&self, _origin: KillOrigin) -> Result<(), AppError> {
        Ok(())
    }
}

/// Port: Broker niepodłączony.
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
    async fn undo_step(&self, _session: &SessionId, _step: u64) -> Result<String, AppError> {
        Err(AppError::unavailable("Cofnięcie kroku", "undo-journal"))
    }
}

/// Okno Brokera (Broker-UI, osobny proces) — jedyny kanał zatwierdzeń (PLAN §8.2). WebView nigdy
/// nie zatwierdza; rdzeń może tylko poprosić o pokazanie karty.
pub trait ApprovalWindow: Send + Sync {
    /// Pokazuje kartę prośby `approval` w oknie Brokera.
    fn present(&self, approval: &str) -> Result<(), AppError>;
}

/// Komunikat trybu deweloperskiego bez Broker-UI.
pub const NEEDS_BROKER_WINDOW: &str = "Ta zmiana wymaga potwierdzenia w oknie Brokera, które nie \
     jest uruchomione (tryb deweloperski bez Broker-UI) — prośba odrzucona.";

/// Brak okna Brokera (tryb deweloperski): każda prośba wymagająca zatwierdzenia jest odrzucana.
pub struct NoApprovalWindow;

impl ApprovalWindow for NoApprovalWindow {
    fn present(&self, _approval: &str) -> Result<(), AppError> {
        Err(AppError::forbidden(NEEDS_BROKER_WINDOW))
    }
}
