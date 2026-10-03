//! Skróty globalne z regułą AltGr (AGENTS.md, PLAN §8.6).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Modyfikatory skrótu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Modifiers {
    /// Ctrl.
    pub ctrl: bool,
    /// Alt.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// Win (Super).
    pub win: bool,
}

/// Klawisz główny skrótu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    /// Litera A–Z (przechowywana wielką).
    Letter(char),
    /// Cyfra 0–9.
    Digit(u8),
    /// Klawisz funkcyjny F1–F24.
    Function(u8),
    /// Spacja.
    Space,
    /// Escape.
    Escape,
}

/// Skrót globalny.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hotkey {
    /// Modyfikatory.
    pub modifiers: Modifiers,
    /// Klawisz.
    pub key: Key,
}

/// Kill-switch `Ctrl+Shift+F12` — zarezerwowany dla jądra.
pub const KILL_SWITCH: Hotkey = Hotkey {
    modifiers: Modifiers {
        ctrl: true,
        alt: false,
        shift: true,
        win: false,
    },
    key: Key::Function(12),
};

/// Litery, których nie wolno łączyć z `Ctrl+Alt(+Shift)` (polski układ: AltGr = Ctrl+Alt).
const ALTGR_LETTERS: [char; 9] = ['A', 'C', 'E', 'L', 'N', 'O', 'S', 'X', 'Z'];

impl Hotkey {
    /// Nowy skrót.
    pub fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }

    /// Reguła AltGr + rezerwacja kill-switcha + wymóg co najmniej jednego modyfikatora
    /// (poza klawiszami funkcyjnymi).
    pub fn validate(&self) -> Result<(), PlatformError> {
        let m = self.modifiers;
        if let Key::Letter(c) = self.key
            && m.ctrl
            && m.alt
            && ALTGR_LETTERS.contains(&c.to_ascii_uppercase())
        {
            return Err(PlatformError::HotkeyRejected(format!(
                "{self}: Ctrl+Alt+{c} koliduje z AltGr (polskie znaki)"
            )));
        }
        if *self == KILL_SWITCH {
            return Err(PlatformError::HotkeyRejected(format!(
                "{self} jest kill-switchem jądra"
            )));
        }
        let has_modifier = m.ctrl || m.alt || m.shift || m.win;
        if !has_modifier && !matches!(self.key, Key::Function(_)) {
            return Err(PlatformError::HotkeyRejected(format!(
                "{self}: brak modyfikatora"
            )));
        }
        Ok(())
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.modifiers;
        for (on, name) in [
            (m.ctrl, "Ctrl"),
            (m.alt, "Alt"),
            (m.shift, "Shift"),
            (m.win, "Win"),
        ] {
            if on {
                write!(f, "{name}+")?;
            }
        }
        match self.key {
            Key::Letter(c) => write!(f, "{}", c.to_ascii_uppercase()),
            Key::Digit(d) => write!(f, "{d}"),
            Key::Function(n) => write!(f, "F{n}"),
            Key::Space => f.write_str("Space"),
            Key::Escape => f.write_str("Esc"),
        }
    }
}

/// Pochodzenie naciśnięcia skrótu globalnego według hooka `WH_KEYBOARD_LL` (flaga
/// `LLKHF_INJECTED` przy wciśnięciu klawisza głównego i modyfikatorów; przegląd #2, P2-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyPressOrigin {
    /// Wszystkie klawisze kombinacji z fizycznej klawiatury.
    Physical,
    /// Którykolwiek klawisz kombinacji wstrzyknięty (`SendInput`, UI Automation).
    Injected,
    /// Hook nie widział klawiszy (brak hooka, okno administratora na pierwszym planie — UIPI).
    Unknown,
}

impl HotkeyPressOrigin {
    /// Czy przyjąć naciśnięcie skrótu Alfy. Kill-switch zawsze (fałszywe przyjęcie jest
    /// bezpieczne — zatrzymuje agentki). Pozostałe (szybkie pytanie, PTT) tylko z wejścia
    /// fizycznego: wstrzyknięte = ignorowane (agentka nie otworzy okna Alfy ani mikrofonu, by
    /// „usłyszeć” własny TTS jako polecenie). Nieznane = przyjęte tylko, gdy na pierwszym planie
    /// jest okno podniesione (UIPI blokuje wtedy `SendInput` procesów zwykłej integralności, więc
    /// naciśnięcie jest fizyczne); inaczej odrzucone (fail-closed).
    pub fn admits(self, hotkey: &Hotkey, foreground_elevated: bool) -> bool {
        *hotkey == KILL_SWITCH
            || match self {
                Self::Physical => true,
                Self::Injected => false,
                Self::Unknown => foreground_elevated,
            }
    }
}

/// Identyfikator zarejestrowanego skrótu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HotkeyId(pub u32);

/// Zdarzenie skrótu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyEvent {
    /// Który skrót.
    pub id: HotkeyId,
    /// Czy naciśnięcie (`true`) czy puszczenie (`false`) — puszczenie używa PTT.
    pub pressed: bool,
}

/// Port skrótów globalnych. Odbiór zdarzeń przez odpytywanie kolejki (bez wątków w kontrakcie).
pub trait HotkeyPort: Send + Sync {
    /// Rejestruje skrót po walidacji reguły AltGr.
    fn register(&self, hotkey: Hotkey) -> Result<HotkeyId, PlatformError>;

    /// Wyrejestrowuje skrót.
    fn unregister(&self, id: HotkeyId) -> Result<(), PlatformError>;

    /// Pobiera oczekujące zdarzenia (FIFO) i opróżnia kolejkę.
    fn drain_events(&self) -> Vec<HotkeyEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hk(ctrl: bool, alt: bool, shift: bool, key: Key) -> Hotkey {
        Hotkey::new(
            Modifiers {
                ctrl,
                alt,
                shift,
                win: false,
            },
            key,
        )
    }

    #[test]
    fn altgr_rule_rejects_all_polish_letters() {
        for c in ALTGR_LETTERS {
            assert!(
                hk(true, true, false, Key::Letter(c)).validate().is_err(),
                "{c}"
            );
            assert!(
                hk(true, true, true, Key::Letter(c.to_ascii_lowercase()))
                    .validate()
                    .is_err()
            );
        }
        assert!(hk(true, true, false, Key::Letter('B')).validate().is_ok());
        assert!(hk(true, false, false, Key::Letter('S')).validate().is_ok());
    }

    #[test]
    fn injected_presses_are_ignored_except_kill_switch() {
        let quick = hk(true, true, false, Key::Space);
        for elevated in [false, true] {
            assert!(HotkeyPressOrigin::Physical.admits(&quick, elevated));
            assert!(!HotkeyPressOrigin::Injected.admits(&quick, elevated));
            assert!(HotkeyPressOrigin::Injected.admits(&KILL_SWITCH, elevated));
            assert!(HotkeyPressOrigin::Unknown.admits(&KILL_SWITCH, elevated));
        }
        assert!(!HotkeyPressOrigin::Unknown.admits(&quick, false));
        assert!(HotkeyPressOrigin::Unknown.admits(&quick, true));
    }

    #[test]
    fn kill_switch_reserved_and_display() {
        assert!(KILL_SWITCH.validate().is_err());
        assert_eq!(KILL_SWITCH.to_string(), "Ctrl+Shift+F12");
        assert!(
            hk(false, false, false, Key::Letter('A'))
                .validate()
                .is_err()
        );
        assert!(hk(false, false, false, Key::Function(5)).validate().is_ok());
        assert_eq!(
            hk(true, true, false, Key::Space).to_string(),
            "Ctrl+Alt+Space"
        );
    }
}
