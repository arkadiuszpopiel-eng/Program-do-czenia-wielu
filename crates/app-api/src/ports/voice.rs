//! Port głosu (`voice-audio`, `voice-tts`, potok `voice-pipeline`) i czat głosowy, który dla
//! potoku implementuje rdzeń: tura z mowy trafia do tej samej sesji co tekst (historia
//! append-only), a odpowiedź przerwana przez użytkownika dostaje fakt „usłyszany prefiks".

use std::sync::Arc;

use async_trait::async_trait;
use providers_contract::CancellationToken;
use sessions_contract::{SessionId, TurnId};
use tokio::sync::mpsc;

use crate::dto::{
    AudioDevice, DictationAction, LocalizedText, ReadAction, SpeakerAction, VoiceFeatures,
    VoiceStatus, WakeAction,
};
use crate::error::AppError;

/// Tekst próbki głosu agentki (Ustawienia → Głos → „Odsłuchaj").
pub const VOICE_PREVIEW_TEXT: &str = "Cześć, tak brzmi mój głos. Powiedz, w czym mogę pomóc.";

/// Komunikat „głos niedostępny" (brak modeli albo sidecarów).
pub fn voice_unavailable_reason() -> LocalizedText {
    LocalizedText::new(
        "Głos niedostępny: pobierz modele w Ustawieniach → Głos.",
        "Voice unavailable: download the models in Settings → Voice.",
    )
}

/// Tura odpowiedzi głosowej w sesji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceTurnRef {
    /// Sesja.
    pub session: SessionId,
    /// Tura agentki (zarezerwowana przy starcie generacji).
    pub turn: TurnId,
}

/// Fragment odpowiedzi dla potoku.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceChunk {
    /// Tekst (markdown — kanał mówiony wydziela `voice-persona`).
    Text(String),
    /// Koniec odpowiedzi.
    Done,
    /// Błąd (PL).
    Failed(String),
}

/// Rozpoczęta odpowiedź głosowa: tura + strumień tekstu.
#[derive(Debug)]
pub struct VoiceTurn {
    /// Tura agentki.
    pub turn: VoiceTurnRef,
    /// Tekst odpowiedzi (kończy się `Done`/`Failed`).
    pub chunks: mpsc::UnboundedReceiver<VoiceChunk>,
}

/// Pochodzenie tury głosowej dla klasyfikatora ryzyka i Brokera (F5): pewność STT wypowiedzi
/// i wynik weryfikacji właściciela (`voice-speaker`). Domyślnie — pewność nieznana
/// (rdzeń przyjmuje wartość ostrożną) i głos **niezweryfikowany** (fail-closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VoiceTurnOrigin {
    /// Pewność finalu STT (‰); `None` — nieznana.
    pub stt_confidence_permille: Option<u16>,
    /// Głos właściciela zweryfikowany progiem ścisłym.
    pub speaker_verified: bool,
}

/// Czat dla rozmowy głosowej (implementuje `app-core`).
#[async_trait]
pub trait VoiceChat: Send + Sync {
    /// Tura użytkownika z mowy (aktywna sesja; bez niej — nowa sesja głosowa) i odpowiedź
    /// agentki `persona`; anulowanie `cancel` (barge-in, „stop") przerywa generowanie.
    /// `origin` trafia do faktów Brokera (`CommandOrigin::UserVoice`).
    async fn voice_turn(
        &self,
        persona: &str,
        text: &str,
        origin: VoiceTurnOrigin,
        cancel: CancellationToken,
    ) -> Result<VoiceTurn, AppError>;
    /// Zamyka turę: `heard = Some((prefiks, przybliżony))` — odpowiedź przerwana, w historii
    /// zostaje fakt „usłyszany prefiks" (append-only, raz).
    async fn voice_finish(&self, turn: VoiceTurnRef, heard: Option<(String, bool)>);
    /// „Stop wszystko" głosem — kill-switch (ten sam co `Ctrl+Shift+F12`).
    async fn kill_switch(&self);
    /// „Anuluj" głosem — zatrzymuje zadanie agentki w aktywnej sesji.
    async fn cancel_task(&self);
}

/// Głos — moduły `voice-*` (audio, TTS; rozmowa głosowa — potok `voice-pipeline`).
#[async_trait]
pub trait VoicePort: Send + Sync {
    /// Urządzenia wejściowe; `None` = użyj listy z `device-profile`.
    async fn devices(&self) -> Result<Option<Vec<AudioDevice>>, AppError>;
    /// Start testu mikrofonu (poziomy przez zdarzenie `MicLevel`).
    async fn start_mic_test(&self, device: Option<String>) -> Result<(), AppError>;
    /// Stop testu mikrofonu.
    async fn stop_mic_test(&self) -> Result<(), AppError>;
    /// Mikrofon wł./wył. (tryb rozmowy).
    async fn set_mic_enabled(&self, enabled: bool) -> Result<(), AppError>;
    /// Wyciszenie.
    async fn set_muted(&self, muted: bool) -> Result<(), AppError>;
    /// Stop mowy.
    async fn stop_speech(&self) -> Result<(), AppError>;
    /// Czytanie tekstu głosem agentki.
    async fn read_aloud(&self, agent: &str, text: &str) -> Result<(), AppError>;
    /// Mówienie z przytrzymaniem (PTT) — wciśnięcie/puszczenie.
    async fn ptt(&self, _pressed: bool) -> Result<(), AppError> {
        Err(AppError::unavailable(
            "Mówienie z przytrzymaniem (PTT)",
            "voice-pipeline",
        ))
    }
    /// Stan trybu głosowego.
    async fn status(&self) -> VoiceStatus {
        VoiceStatus::unavailable(voice_unavailable_reason(), vec!["voice-pipeline".into()])
    }
    /// Próbka głosu agentki (głosy v0).
    async fn preview(&self, agent: &str) -> Result<(), AppError> {
        self.read_aloud(agent, VOICE_PREVIEW_TEXT).await
    }
    /// Podpina czat (rdzeń po zbudowaniu) — odpowiedzi rozmowy głosowej idą do tej samej sesji.
    fn attach(&self, _chat: Arc<dyn VoiceChat>) {}
    /// Głos rozszerzony F5: stan słów wywoławczych, weryfikacji właściciela, dyktowania, czytania.
    async fn features(&self) -> VoiceFeatures {
        VoiceFeatures::unavailable(voice_unavailable_reason())
    }
    /// Słowa wywoławcze: konfiguracja (jawne włączenie), test, „nie przeszkadzać”.
    async fn wake(&self, _action: WakeAction) -> Result<VoiceFeatures, AppError> {
        Err(AppError::unavailable("Słowa wywoławcze", "voice-wake"))
    }
    /// Kreator rejestracji głosu i weryfikacja właściciela.
    async fn speaker(&self, _action: SpeakerAction) -> Result<VoiceFeatures, AppError> {
        Err(AppError::unavailable(
            "Rozpoznawanie głosu",
            "voice-speaker",
        ))
    }
    /// Dyktowanie do aplikacji na pierwszym planie.
    async fn dictation(&self, _action: DictationAction) -> Result<VoiceFeatures, AppError> {
        Err(AppError::unavailable("Dyktowanie", "voice-dictation"))
    }
    /// Czytanie na głos zaznaczenia, dokumentu albo schowka.
    async fn read(&self, _action: ReadAction) -> Result<VoiceFeatures, AppError> {
        Err(AppError::unavailable("Czytanie na głos", "voice-readaloud"))
    }
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
