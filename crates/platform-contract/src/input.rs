//! Rozpoznawanie wejścia wstrzykniętego (SendInput / `keybd_event` / UI Automation) —
//! podstawa dowodu fizycznego wejścia w Broker-UI (PLAN §8.2, ADR 3, THREAT_MODEL S11).
//!
//! Dwa niezależne źródła: flagi hooków niskiego poziomu (`WH_KEYBOARD_LL`: `LLKHF_INJECTED`,
//! `LLKHF_LOWER_IL_INJECTED`; `WH_MOUSE_LL`: `LLMHF_INJECTED`, `LLMHF_LOWER_IL_INJECTED`) oraz
//! pochodzenie bieżącego komunikatu (`GetCurrentInputMessageSource` → `IMO_*`). Decyzja jest
//! fail-closed: wejście uznajemy za fizyczne tylko, gdy oba źródła na to wskazują albo komunikat
//! jest sprzętowy i hook nie widział wstrzyknięcia.

use serde::{Deserialize, Serialize};

/// `KBDLLHOOKSTRUCT.flags`: zdarzenie wstrzyknięte.
pub const LLKHF_INJECTED: u32 = 0x10;
/// `KBDLLHOOKSTRUCT.flags`: wstrzyknięte z procesu o niższej integralności.
pub const LLKHF_LOWER_IL_INJECTED: u32 = 0x02;
/// `MSLLHOOKSTRUCT.flags`: zdarzenie wstrzyknięte.
pub const LLMHF_INJECTED: u32 = 0x01;
/// `MSLLHOOKSTRUCT.flags`: wstrzyknięte z procesu o niższej integralności.
pub const LLMHF_LOWER_IL_INJECTED: u32 = 0x02;
/// Jak długo obserwacja hooka dotyczy komunikatu w oknie (ms).
pub const HOOK_WINDOW_MS: u64 = 1_000;

/// Urządzenie wejścia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputDevice {
    /// Klawiatura.
    Keyboard,
    /// Mysz.
    Mouse,
    /// Dotyk.
    Touch,
    /// Pióro.
    Pen,
    /// Nieznane (`IMDT_UNAVAILABLE`).
    Unknown,
}

impl InputDevice {
    /// Z `INPUT_MESSAGE_DEVICE_TYPE` (`IMDT_*`).
    pub fn from_imdt(device_type: u32) -> Self {
        match device_type {
            1 => Self::Keyboard,
            2 => Self::Mouse,
            4 => Self::Touch,
            8 => Self::Pen,
            _ => Self::Unknown,
        }
    }
}

/// Pochodzenie zdarzenia według flag hooka niskiego poziomu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookOrigin {
    /// Sprzęt (brak flag wstrzyknięcia).
    Hardware,
    /// Wstrzyknięte (SendInput itp.).
    Injected,
    /// Wstrzyknięte z procesu o niższej integralności (np. agentka → Broker-UI).
    InjectedFromLowerIntegrity,
}

impl HookOrigin {
    /// Z `KBDLLHOOKSTRUCT.flags`.
    pub fn from_keyboard_flags(flags: u32) -> Self {
        Self::from_bits(
            flags & LLKHF_INJECTED != 0,
            flags & LLKHF_LOWER_IL_INJECTED != 0,
        )
    }

    /// Z `MSLLHOOKSTRUCT.flags`.
    pub fn from_mouse_flags(flags: u32) -> Self {
        Self::from_bits(
            flags & LLMHF_INJECTED != 0,
            flags & LLMHF_LOWER_IL_INJECTED != 0,
        )
    }

    fn from_bits(injected: bool, lower: bool) -> Self {
        match (injected, lower) {
            (_, true) => Self::InjectedFromLowerIntegrity,
            (true, false) => Self::Injected,
            (false, false) => Self::Hardware,
        }
    }

    /// Czy wstrzyknięte (w jakiejkolwiek postaci).
    pub fn is_injected(self) -> bool {
        self != Self::Hardware
    }
}

/// Pochodzenie komunikatu według `GetCurrentInputMessageSource` (`IMO_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageOrigin {
    /// Brak informacji (`IMO_UNAVAILABLE`) albo wywołanie nieudane.
    Unavailable,
    /// Sprzęt (`IMO_HARDWARE`).
    Hardware,
    /// Wstrzyknięte (`IMO_INJECTED`).
    Injected,
    /// Wygenerowane przez system (`IMO_SYSTEM`).
    System,
}

impl MessageOrigin {
    /// Z `INPUT_MESSAGE_ORIGIN_ID`.
    pub fn from_imo(origin_id: u32) -> Self {
        match origin_id {
            1 => Self::Hardware,
            2 => Self::Injected,
            4 => Self::System,
            _ => Self::Unavailable,
        }
    }
}

/// Ostatnia obserwacja hooka dla urządzenia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookObservation {
    /// Pochodzenie.
    pub origin: HookOrigin,
    /// Chwila (ms).
    pub at_ms: u64,
}

/// Decyzja fail-closed: czy wejście trzeba traktować jako wstrzyknięte.
///
/// - komunikat `Injected` → wstrzyknięte,
/// - komunikat `Hardware` → wstrzyknięte tylko, gdy świeża obserwacja hooka mówi „wstrzyknięte”,
/// - komunikat `System`/`Unavailable` → fizyczne tylko przy świeżej obserwacji hooka „sprzęt”.
pub fn input_is_injected(
    message: MessageOrigin,
    hook: Option<HookObservation>,
    at_ms: u64,
) -> bool {
    let fresh = hook.filter(|h| at_ms.saturating_sub(h.at_ms) <= HOOK_WINDOW_MS);
    match message {
        MessageOrigin::Injected => true,
        MessageOrigin::Hardware => fresh.is_some_and(|h| h.origin.is_injected()),
        MessageOrigin::System | MessageOrigin::Unavailable => {
            fresh.is_none_or(|h| h.origin.is_injected())
        }
    }
}

/// Próbka wejścia przekazywana do logiki Broker-UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputSample {
    /// Urządzenie.
    pub device: InputDevice,
    /// Czy wstrzyknięte (wynik [`input_is_injected`]).
    pub injected: bool,
    /// Chwila (ms od epoki UNIX).
    pub at_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_flags_map_to_origin() {
        assert_eq!(HookOrigin::from_keyboard_flags(0), HookOrigin::Hardware);
        assert_eq!(
            HookOrigin::from_keyboard_flags(0x01 | 0x20 | 0x80),
            HookOrigin::Hardware,
            "LLKHF_EXTENDED/ALTDOWN/UP to nie wstrzyknięcie"
        );
        assert_eq!(
            HookOrigin::from_keyboard_flags(LLKHF_INJECTED),
            HookOrigin::Injected
        );
        assert_eq!(
            HookOrigin::from_keyboard_flags(LLKHF_INJECTED | LLKHF_LOWER_IL_INJECTED),
            HookOrigin::InjectedFromLowerIntegrity
        );
        assert_eq!(
            HookOrigin::from_mouse_flags(LLMHF_INJECTED),
            HookOrigin::Injected
        );
        assert_eq!(
            HookOrigin::from_mouse_flags(LLMHF_LOWER_IL_INJECTED),
            HookOrigin::InjectedFromLowerIntegrity
        );
        assert!(!HookOrigin::from_mouse_flags(0).is_injected());
        assert_eq!(MessageOrigin::from_imo(1), MessageOrigin::Hardware);
        assert_eq!(MessageOrigin::from_imo(2), MessageOrigin::Injected);
        assert_eq!(MessageOrigin::from_imo(4), MessageOrigin::System);
        assert_eq!(MessageOrigin::from_imo(3), MessageOrigin::Unavailable);
        assert_eq!(InputDevice::from_imdt(2), InputDevice::Mouse);
        assert_eq!(InputDevice::from_imdt(1), InputDevice::Keyboard);
        assert_eq!(InputDevice::from_imdt(4), InputDevice::Touch);
        assert_eq!(InputDevice::from_imdt(8), InputDevice::Pen);
        assert_eq!(InputDevice::from_imdt(0), InputDevice::Unknown);
    }

    #[test]
    fn decision_is_fail_closed() {
        let hw = |at_ms| HookObservation {
            origin: HookOrigin::Hardware,
            at_ms,
        };
        let inj = |at_ms| HookObservation {
            origin: HookOrigin::Injected,
            at_ms,
        };
        use MessageOrigin::*;
        assert!(input_is_injected(Injected, Some(hw(100)), 100));
        assert!(!input_is_injected(Hardware, None, 100));
        assert!(!input_is_injected(Hardware, Some(hw(100)), 100));
        assert!(input_is_injected(Hardware, Some(inj(100)), 100));
        assert!(
            !input_is_injected(Hardware, Some(inj(0)), 5_000),
            "stara obserwacja nie dotyczy"
        );
        assert!(input_is_injected(Unavailable, None, 100));
        assert!(input_is_injected(System, Some(inj(90)), 100));
        assert!(!input_is_injected(System, Some(hw(90)), 100));
        assert!(
            input_is_injected(Unavailable, Some(hw(0)), 5_000),
            "brak świeżej obserwacji = wstrzyknięte"
        );
    }
}
