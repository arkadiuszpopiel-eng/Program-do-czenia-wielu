//! Reguła „agentka nie wpisuje haseł” dla wejścia syntetycznego (przegląd bezpieczeństwa #2,
//! P2-03): tekst, dyktowanie i skróty edytujące (litery, cyfry, spacja, Backspace, Delete,
//! Insert — także `Ctrl+V`/`Shift+Insert`) nigdy nie trafiają do pola hasła (UIA `IsPassword`).
//!
//! Zasada **fail-closed**: wpisywanie jest dozwolone tylko, gdy element z fokusem klawiatury
//! został ustalony i nie jest polem hasła. Fokus nieznany (UIA zawiesiło się, element poza oknem
//! celu, implementacja portu bez odczytu fokusu) = odmowa. Sprawdzenie robi wspólne
//! [`crate::execute_input`] przed **każdą** paczką edytującą (fokus może przejść do pola hasła
//! w trakcie wpisywania), a narzędzia agentek dodatkowo przed wysłaniem planu.
//!
//! Testy: `platform-fake/tests/review_contract.rs` (limit rozmiaru crate'a kontraktu).

use serde::{Deserialize, Serialize};

use crate::gui::GuiError;
use crate::keys::ChordKey;
use crate::synth::RawInput;
use crate::uia::UiaNode;

/// Element z fokusem klawiatury z punktu widzenia reguły haseł.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusedField {
    /// Ustalony element, który nie jest polem hasła.
    Ordinary,
    /// Pole hasła (`IsPassword`).
    Password,
    /// Nie da się ustalić (błąd/limit czasu UIA, fokus poza oknem celu) — traktowane jak hasło.
    Unknown,
}

impl FocusedField {
    /// Z wyniku `UiaPort::focused` (`Ok(None)` = fokus poza oknem → nieznany).
    pub fn from_lookup(lookup: &Result<Option<UiaNode>, GuiError>) -> Self {
        match lookup {
            Ok(Some(node)) if node.is_password => Self::Password,
            Ok(Some(_)) => Self::Ordinary,
            Ok(None) | Err(_) => Self::Unknown,
        }
    }

    /// Czy wolno wpisywać tekst (`Err(Policy)` dla pola hasła i fokusu nieznanego).
    pub fn check_typing(self) -> Result<(), GuiError> {
        match self {
            Self::Ordinary => Ok(()),
            Self::Password => Err(GuiError::Policy(
                "fokus jest w polu hasła — agentka nie wpisuje haseł (wpisz je sama/sam)".into(),
            )),
            Self::Unknown => Err(GuiError::Policy(
                "nie da się ustalić pola z fokusem (może to być pole hasła) — nic nie wpisuję"
                    .into(),
            )),
        }
    }
}

impl ChordKey {
    /// Czy klawisz zmienia treść pola (znak, spacja, Backspace, Delete, Insert) — w polu hasła
    /// zakazany niezależnie od modyfikatorów (`Ctrl+V` wkleja, `Shift+Insert` też).
    pub fn edits_field(self) -> bool {
        matches!(
            self,
            Self::Letter(_)
                | Self::Digit(_)
                | Self::Space
                | Self::Backspace
                | Self::Delete
                | Self::Insert
        )
    }
}

/// Kody wirtualne klawiszy edytujących (te same co [`ChordKey::edits_field`] + klawiatura
/// numeryczna i znaki OEM — gdyby pojawiły się w surowych zdarzeniach).
fn vk_edits_field(vk: u16) -> bool {
    matches!(vk, 0x08 | 0x20 | 0x2D | 0x2E | 0x30..=0x39 | 0x41..=0x5A | 0x60..=0x6F | 0xBA..=0xE2)
}

/// Czy paczka wpisuje coś do pola z fokusem (jednostki Unicode albo klawisze edytujące).
/// Enter, Tab, strzałki i klawisze funkcyjne same w sobie nie wpisują treści.
pub fn batch_writes_text(events: &[RawInput]) -> bool {
    events.iter().any(|e| match *e {
        RawInput::Unicode { .. } => true,
        RawInput::Key { vk, .. } => vk_edits_field(vk),
        RawInput::MoveTo { .. } | RawInput::Button { .. } | RawInput::Wheel { .. } => false,
    })
}
