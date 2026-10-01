//! Reguły przyjęcia wejścia jako dowodu fizycznego (ADR 3, THREAT_MODEL S11): wejście
//! niewstrzyknięte, z rozpoznanego urządzenia, po pokazaniu karty i przed jej wygaśnięciem,
//! okno na pierwszym planie nieprzerwanie co najmniej [`MIN_FOREGROUND_MS`] (ochrona przed
//! clickjackingiem: obce okno nie „podsunie” karty pod trwające kliknięcie) i niezasłonięte.

use platform_contract::{InputDevice, InputSample};
use safety_broker_contract::InputSource;
use serde::{Deserialize, Serialize};

/// Minimalny czas nieprzerwanego pierwszego planu przed wejściem (ms).
pub const MIN_FOREGROUND_MS: u64 = 500;

/// Powód odrzucenia wejścia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum RejectReason {
    /// Wejście wstrzyknięte (SendInput, UIA, `keybd_event`) — nigdy nie jest dowodem.
    #[error("wejście wstrzyknięte — wymagane fizyczne kliknięcie lub klawisz")]
    Injected,
    /// Nierozpoznane urządzenie wejścia.
    #[error("nierozpoznane urządzenie wejścia")]
    UnknownDevice,
    /// Okno nie jest na pierwszym planie.
    #[error("okno Brokera nie jest aktywne")]
    NotForeground,
    /// Okno za krótko na pierwszym planie.
    #[error("okno dopiero stało się aktywne ({elapsed_ms} ms) — kliknij ponownie")]
    TooSoon {
        /// Ile ms okno było na pierwszym planie.
        elapsed_ms: u64,
    },
    /// Okno zasłonięte innym oknem.
    #[error("okno Brokera jest zasłonięte innym oknem")]
    Occluded,
    /// Wejście sprzed pokazania karty.
    #[error("wejście sprzed pokazania karty")]
    BeforeShown,
    /// Prośba wygasła.
    #[error("prośba wygasła")]
    Expired,
    /// Windows Hello wymagane, ale niedostępne.
    #[error("wymagane Windows Hello jest niedostępne")]
    HelloUnavailable,
    /// Windows Hello anulowane lub nieudane.
    #[error("Windows Hello nie potwierdziło tożsamości")]
    HelloFailed,
}

/// Śledzenie pierwszego planu okna (od kiedy nieprzerwanie aktywne).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ForegroundTracker {
    since: Option<u64>,
}

impl ForegroundTracker {
    /// Okno stało się aktywne (kolejne aktywacje bez dezaktywacji nie przesuwają początku).
    pub fn activated(&mut self, at_ms: u64) {
        if self.since.is_none() {
            self.since = Some(at_ms);
        }
    }

    /// Okno straciło pierwszy plan.
    pub fn deactivated(&mut self) {
        self.since = None;
    }

    /// Od kiedy aktywne.
    pub fn since(&self) -> Option<u64> {
        self.since
    }
}

/// Źródło dowodu dla urządzenia (dotyk i pióro jak kliknięcie; nieznane — brak).
pub fn source_for(device: InputDevice) -> Option<InputSource> {
    match device {
        InputDevice::Keyboard => Some(InputSource::Keyboard),
        InputDevice::Mouse | InputDevice::Touch | InputDevice::Pen => Some(InputSource::MouseClick),
        InputDevice::Unknown => None,
    }
}

/// Czy wejście może zostać dowodem fizycznym dla karty pokazanej w `shown_at_ms`.
pub fn check_input(
    fg: &ForegroundTracker,
    input: &InputSample,
    occluded: bool,
    shown_at_ms: u64,
    expires_at_ms: u64,
) -> Result<InputSource, RejectReason> {
    if input.injected {
        return Err(RejectReason::Injected);
    }
    let source = source_for(input.device).ok_or(RejectReason::UnknownDevice)?;
    if input.at_ms < shown_at_ms {
        return Err(RejectReason::BeforeShown);
    }
    if input.at_ms >= expires_at_ms {
        return Err(RejectReason::Expired);
    }
    let since = fg.since().ok_or(RejectReason::NotForeground)?;
    let elapsed_ms = input.at_ms.saturating_sub(since);
    if input.at_ms < since || elapsed_ms < MIN_FOREGROUND_MS {
        return Err(RejectReason::TooSoon { elapsed_ms });
    }
    if occluded {
        return Err(RejectReason::Occluded);
    }
    Ok(source)
}
