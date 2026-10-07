//! Kontrakt modułu `voice-s2s` (docs/modules/voice-s2s/SPEC.md, PLAN §6.2, §6.3 profil C,
//! §6.5): natywny speech-to-speech w chmurze („tryb szybkiej rozmowy”) — OpenAI Realtime /
//! Gemini Live. Głosy = presety dostawcy przypisane do agentek (nie własne biblie głosu).
//!
//! - Prywatność: sesja z tagiem `Private` nigdy nie łączy się z chmurą ([`S2sCfg::validate`]);
//!   każda wysłana porcja audio jest raportowana (`voice.s2s.audio_sent` — ekran „co poszło
//!   do chmury”).
//! - Przerwanie: `cancel_response` + **natywne obcięcie** po stronie dostawcy
//!   (`conversation.item.truncate` do miejsca, które użytkownik usłyszał — [`truncate_point`]);
//!   historia nie dostaje notki (`InterruptionRendering::NativeTruncate`).
//! - v0: kontrakt + atrapa; adapter chmurowy (WebSocket/WebRTC, klucze z `accounts-hub`) — później.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::{Event, EventKind, Level};
use personas_contract::PersonaId;
use providers_contract::{InterruptionRendering, PrivacyTag};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_audio_contract::Frame;

/// Sesja rozpoczęta.
pub const EVENT_SESSION_STARTED: &str = "voice.s2s.session_started";
/// Audio wysłane do chmury (ms).
pub const EVENT_AUDIO_SENT: &str = "voice.s2s.audio_sent";
/// Odpowiedź obcięta u dostawcy (przerwanie).
pub const EVENT_TRUNCATED: &str = "voice.s2s.truncated";
/// Sesja zamknięta.
pub const EVENT_SESSION_CLOSED: &str = "voice.s2s.session_closed";

/// Dostawca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum S2sProvider {
    /// OpenAI Realtime (natywne `conversation.item.truncate`).
    OpenAiRealtime,
    /// Gemini Live.
    GeminiLive,
}

impl S2sProvider {
    /// Jak dostawca renderuje przerwanie.
    pub fn interruption(self) -> InterruptionRendering {
        match self {
            Self::OpenAiRealtime => InterruptionRendering::NativeTruncate,
            Self::GeminiLive => InterruptionRendering::AppendNote,
        }
    }
}

/// Konfiguracja (`[voice.s2s]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct S2sCfg {
    /// Dostawca.
    pub provider: S2sProvider,
    /// Konto (`accounts-hub`; klucz nigdy w konfiguracji).
    pub account: String,
    /// Model.
    pub model: String,
    /// Preset głosu dostawcy per agentka.
    pub voices: Vec<(PersonaId, String)>,
    /// Tag prywatności sesji czatu.
    pub privacy: PrivacyTag,
    /// Częstotliwość audio (Realtime: 24 kHz PCM16).
    pub sample_rate: u32,
    /// Limit długości sesji (koszt).
    pub max_session_minutes: u32,
}

impl S2sCfg {
    /// Walidacja: sesja prywatna → odmowa (audio nie idzie do chmury), głos dla agentki,
    /// częstotliwość 16/24 kHz, limit 1–120 min.
    pub fn validate(&self, persona: &PersonaId) -> Result<(), S2sError> {
        if self.privacy == PrivacyTag::Private {
            return Err(S2sError::PrivacyBlocked);
        }
        if self.account.trim().is_empty() || self.model.trim().is_empty() {
            return Err(S2sError::InvalidConfig("konto i model są wymagane".into()));
        }
        if !self.voices.iter().any(|(p, _)| p == persona) {
            return Err(S2sError::InvalidConfig(format!(
                "brak presetu głosu dla agentki {persona}"
            )));
        }
        if ![16_000, 24_000].contains(&self.sample_rate)
            || !(1..=120).contains(&self.max_session_minutes)
        {
            return Err(S2sError::InvalidConfig(
                "audio 16/24 kHz, sesja 1–120 min".into(),
            ));
        }
        Ok(())
    }
}

/// Identyfikator elementu rozmowy u dostawcy (odpowiedź asystentki).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ItemId(pub String);

/// Kto mówi w transkrypcji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum S2sRole {
    /// Użytkownik (transkrypcja wejścia).
    User,
    /// Asystentka.
    Assistant,
}

/// Zdarzenie sesji (strumień do potoku/UI).
#[derive(Debug, Clone, PartialEq)]
pub enum S2sEvent {
    /// Dostawca wykrył mowę użytkownika (VAD serwera) — sygnał barge-in.
    UserSpeechStarted,
    /// Fragment audio odpowiedzi.
    AudioDelta {
        /// Element odpowiedzi.
        item: ItemId,
        /// Audio (mono, `sample_rate`).
        audio: Frame,
    },
    /// Fragment transkrypcji.
    Transcript {
        /// Kto.
        role: S2sRole,
        /// Tekst.
        text: String,
        /// Final.
        is_final: bool,
    },
    /// Odpowiedź zakończona.
    ResponseDone {
        /// Element.
        item: ItemId,
    },
    /// Błąd dostawcy (PL).
    Error(String),
}

/// Zdarzenia magistrali (bez audio i bez treści).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum S2sBusEvent {
    /// `voice.s2s.session_started`.
    SessionStarted {
        /// Dostawca.
        provider: S2sProvider,
        /// Model.
        model: String,
        /// Agentka.
        persona: PersonaId,
    },
    /// `voice.s2s.audio_sent`.
    AudioSent {
        /// Dostawca.
        provider: S2sProvider,
        /// Długość audio (ms).
        audio_ms: u32,
    },
    /// `voice.s2s.truncated`.
    Truncated {
        /// Element.
        item: ItemId,
        /// Ile audio odpowiedzi użytkownik usłyszał (ms).
        audio_end_ms: u32,
    },
    /// `voice.s2s.session_closed`.
    SessionClosed {
        /// Łącznie wysłanego audio (ms).
        audio_sent_ms: u64,
    },
}

impl S2sBusEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            Self::SessionStarted { .. } => EVENT_SESSION_STARTED,
            Self::AudioSent { .. } => EVENT_AUDIO_SENT,
            Self::Truncated { .. } => EVENT_TRUNCATED,
            Self::SessionClosed { .. } => EVENT_SESSION_CLOSED,
        }
    }

    /// Zdarzenie magistrali.
    pub fn to_bus_event(&self) -> Event {
        let level = match self {
            Self::AudioSent { .. } => Level::Debug,
            _ => Level::Info,
        };
        Event::new(
            EventKind::Custom(self.name().to_owned()),
            level,
            serde_json::to_value(self).unwrap_or_default(),
        )
    }
}

/// Błędy.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum S2sError {
    /// Sesja prywatna — audio nie może trafić do chmury.
    #[error("sesja prywatna — tryb rozmowy w chmurze jest wyłączony")]
    PrivacyBlocked,
    /// Niepoprawna konfiguracja.
    #[error("niepoprawna konfiguracja S2S: {0}")]
    InvalidConfig(String),
    /// Sesja zamknięta.
    #[error("sesja S2S zamknięta")]
    Closed,
    /// Nieznany element rozmowy (obcięcie elementu, którego audio nie dotarło).
    #[error("nieznany element rozmowy: {0}")]
    UnknownItem(String),
    /// Dostawca nie obsługuje operacji (np. natywnego obcięcia — wtedy potok dopisuje notkę).
    #[error("dostawca S2S nie obsługuje tej operacji")]
    Unsupported,
    /// Dostawca (sieć, limit, odmowa).
    #[error("dostawca S2S: {0}")]
    Provider(String),
}

/// Miejsce obcięcia odpowiedzi: ile ms audio użytkownik naprawdę usłyszał (odtworzone próbki
/// minus opóźnienie urządzenia — ta sama hierarchia co „usłyszany prefiks”, VOICE.md §7).
pub fn truncate_point(played_samples: u64, sample_rate: u32, device_latency: Duration) -> u32 {
    let played_ms = played_samples * 1000 / u64::from(sample_rate.max(1));
    let heard = played_ms.saturating_sub(u64::try_from(device_latency.as_millis()).unwrap_or(0));
    u32::try_from(heard).unwrap_or(u32::MAX)
}

/// Sesja S2S.
#[async_trait]
pub trait S2sSession: Send {
    /// Audio mikrofonu (po AEC; mono, `sample_rate` z konfiguracji — inny format →
    /// `InvalidConfig`). Każde wywołanie raportuje `AudioSent`.
    async fn send_audio(&mut self, frame: &Frame) -> Result<(), S2sError>;
    /// Koniec tury użytkownika (PTT; przy VAD serwera — opcjonalne). Pusty bufor → `Provider`.
    async fn commit_turn(&mut self) -> Result<(), S2sError>;
    /// Przerwanie bieżącej odpowiedzi (barge-in, „stop”): po nim żadne `AudioDelta` tego
    /// elementu nie przychodzi. Bez trwającej odpowiedzi — no-op.
    async fn cancel_response(&mut self) -> Result<(), S2sError>;
    /// Natywne obcięcie elementu do `audio_end_ms` (to, co usłyszał użytkownik; przycinane do
    /// długości dostarczonego audio) — zdarzenie `Truncated`. Element bez dostarczonego audio →
    /// `UnknownItem`; dostawca bez natywnego obcięcia → `Unsupported` (potok dopisuje notkę).
    async fn truncate(&mut self, item: &ItemId, audio_end_ms: u32) -> Result<(), S2sError>;
    /// Zdarzenia sesji od ostatniego odczytu.
    fn poll(&mut self) -> Vec<S2sEvent>;
    /// Zdarzenia magistrali od ostatniego odczytu.
    fn take_bus_events(&mut self) -> Vec<S2sBusEvent>;
    /// Zamyka sesję (idempotentnie; jedno `SessionClosed`). Potem operacje → `Closed`.
    async fn close(&mut self);
}

/// Klient S2S (fabryka sesji).
#[async_trait]
pub trait S2sClient: Send + Sync {
    /// Otwiera sesję dla agentki z instrukcją systemową (sprawdza [`S2sCfg::validate`]).
    async fn connect(
        &self,
        cfg: &S2sCfg,
        persona: &PersonaId,
        instructions: &str,
    ) -> Result<Box<dyn S2sSession>, S2sError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> S2sCfg {
        S2sCfg {
            provider: S2sProvider::OpenAiRealtime,
            account: "openai-1".into(),
            model: "gpt-realtime".into(),
            voices: vec![(PersonaId::alfa(), "marin".into())],
            privacy: PrivacyTag::Normal,
            sample_rate: 24_000,
            max_session_minutes: 30,
        }
    }

    #[test]
    fn config_privacy_and_truncation() {
        assert!(cfg().validate(&PersonaId::alfa()).is_ok());
        let private = S2sCfg {
            privacy: PrivacyTag::Private,
            ..cfg()
        };
        assert_eq!(
            private.validate(&PersonaId::alfa()),
            Err(S2sError::PrivacyBlocked)
        );
        assert!(cfg().validate(&PersonaId::beta()).is_err());
        assert!(
            S2sCfg {
                sample_rate: 44_100,
                ..cfg()
            }
            .validate(&PersonaId::alfa())
            .is_err()
        );
        assert_eq!(
            S2sProvider::OpenAiRealtime.interruption(),
            InterruptionRendering::NativeTruncate
        );
        assert_eq!(
            S2sProvider::GeminiLive.interruption(),
            InterruptionRendering::AppendNote
        );
        assert_eq!(
            truncate_point(48_000, 24_000, Duration::from_millis(120)),
            1_880
        );
        assert_eq!(truncate_point(100, 24_000, Duration::from_millis(120)), 0);
        for e in [
            S2sBusEvent::SessionStarted {
                provider: S2sProvider::GeminiLive,
                model: "m".into(),
                persona: PersonaId::alfa(),
            },
            S2sBusEvent::AudioSent {
                provider: S2sProvider::OpenAiRealtime,
                audio_ms: 20,
            },
            S2sBusEvent::Truncated {
                item: ItemId("i".into()),
                audio_end_ms: 5,
            },
            S2sBusEvent::SessionClosed { audio_sent_ms: 0 },
        ] {
            assert_eq!(e.to_bus_event().kind.as_str(), e.name());
        }
    }
}
