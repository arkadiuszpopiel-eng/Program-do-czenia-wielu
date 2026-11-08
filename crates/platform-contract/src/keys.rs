//! Skróty klawiszowe dla wejścia syntetycznego (`Ctrl+S`, `Alt+F4`, `Enter`): parsowanie,
//! kody wirtualne Windows (`VK_*`, neutralne dane — atrapa ich nie interpretuje) i zakaz skrótów
//! działających poza aplikacją docelową (klawisz Win, przełączanie zadań, Menedżer zadań,
//! kill-switch Jądra) — agentka steruje tylko oknem, na które ma `gui.control(aplikacja)`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::gui::GuiError;

/// `VK_CONTROL`.
pub const VK_CONTROL: u16 = 0x11;
/// `VK_MENU` (Alt).
pub const VK_MENU: u16 = 0x12;
/// `VK_SHIFT`.
pub const VK_SHIFT: u16 = 0x10;
/// `VK_LWIN`.
pub const VK_LWIN: u16 = 0x5B;
/// `VK_RETURN`.
pub const VK_RETURN: u16 = 0x0D;
/// `VK_TAB`.
pub const VK_TAB: u16 = 0x09;

/// Klawisz główny skrótu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChordKey {
    /// Litera A–Z (wielka).
    Letter(char),
    /// Cyfra 0–9.
    Digit(u8),
    /// F1–F24.
    Function(u8),
    /// Enter.
    Enter,
    /// Tab.
    Tab,
    /// Esc.
    Escape,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Insert.
    Insert,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Strzałka w lewo.
    Left,
    /// Strzałka w prawo.
    Right,
    /// Strzałka w górę.
    Up,
    /// Strzałka w dół.
    Down,
    /// Spacja.
    Space,
}

const NAMED: [(&str, ChordKey); 22] = [
    ("enter", ChordKey::Enter),
    ("return", ChordKey::Enter),
    ("tab", ChordKey::Tab),
    ("esc", ChordKey::Escape),
    ("escape", ChordKey::Escape),
    ("backspace", ChordKey::Backspace),
    ("delete", ChordKey::Delete),
    ("del", ChordKey::Delete),
    ("insert", ChordKey::Insert),
    ("ins", ChordKey::Insert),
    ("home", ChordKey::Home),
    ("end", ChordKey::End),
    ("pageup", ChordKey::PageUp),
    ("pgup", ChordKey::PageUp),
    ("pagedown", ChordKey::PageDown),
    ("pgdn", ChordKey::PageDown),
    ("left", ChordKey::Left),
    ("right", ChordKey::Right),
    ("up", ChordKey::Up),
    ("down", ChordKey::Down),
    ("space", ChordKey::Space),
    ("spacja", ChordKey::Space),
];

impl ChordKey {
    fn parse(text: &str) -> Option<Self> {
        let t = text.trim().to_lowercase().replace([' ', '_', '-'], "");
        let mut chars = t.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) if c.is_ascii_alphabetic() => {
                return Some(Self::Letter(c.to_ascii_uppercase()));
            }
            (Some(c), None) if c.is_ascii_digit() => {
                return c
                    .to_digit(10)
                    .and_then(|d| u8::try_from(d).ok())
                    .map(Self::Digit);
            }
            _ => {}
        }
        if let Some(n) = t.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
            return (1..=24).contains(&n).then_some(Self::Function(n));
        }
        NAMED.iter().find(|(n, _)| *n == t).map(|(_, k)| *k)
    }

    /// Kod wirtualny Windows.
    pub fn vk(self) -> u16 {
        match self {
            Self::Letter(c) => u16::from(u8::try_from(c).unwrap_or(b'A')),
            Self::Digit(d) => 0x30 + u16::from(d.min(9)),
            Self::Function(n) => 0x6F + u16::from(n.clamp(1, 24)),
            Self::Enter => VK_RETURN,
            Self::Tab => VK_TAB,
            Self::Escape => 0x1B,
            Self::Backspace => 0x08,
            Self::Delete => 0x2E,
            Self::Insert => 0x2D,
            Self::Home => 0x24,
            Self::End => 0x23,
            Self::PageUp => 0x21,
            Self::PageDown => 0x22,
            Self::Left => 0x25,
            Self::Up => 0x26,
            Self::Right => 0x27,
            Self::Down => 0x28,
            Self::Space => 0x20,
        }
    }
}

/// Czy klawisz wymaga flagi `KEYEVENTF_EXTENDEDKEY` (blok nawigacji i strzałki).
pub fn is_extended_vk(vk: u16) -> bool {
    matches!(vk, 0x21..=0x28 | 0x2D | 0x2E | VK_LWIN)
}

/// Skrót: modyfikatory + klawisz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyChord {
    /// Ctrl.
    pub ctrl: bool,
    /// Alt.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// Win.
    pub win: bool,
    /// Klawisz.
    pub key: ChordKey,
}

impl KeyChord {
    /// Parsuje `Ctrl+Shift+S`, `alt + f4`, `Enter` (bez wielkości liter).
    pub fn parse(text: &str) -> Result<Self, GuiError> {
        let bad = |why: &str| GuiError::Policy(format!("skrót `{text}`: {why}"));
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (key, mods) = parts.split_last().ok_or_else(|| bad("pusty"))?;
        let mut chord = Self {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            key: ChordKey::parse(key).ok_or_else(|| bad("nieznany klawisz"))?,
        };
        for m in mods {
            let flag = match m.to_lowercase().as_str() {
                "ctrl" | "control" => &mut chord.ctrl,
                "alt" => &mut chord.alt,
                "shift" => &mut chord.shift,
                "win" | "meta" | "super" | "cmd" => &mut chord.win,
                _ => return Err(bad("nieznany modyfikator")),
            };
            if *flag {
                return Err(bad("powtórzony modyfikator"));
            }
            *flag = true;
        }
        Ok(chord)
    }

    /// Powód odmowy, gdy skrót działa poza aplikacją docelową (system, przełączanie zadań,
    /// Menedżer zadań, kill-switch Jądra) — `None` = dozwolony.
    pub fn system_scope(&self) -> Option<&'static str> {
        use ChordKey::{Delete, Escape, Function, Tab};
        let only = |ctrl, alt, shift| self.ctrl == ctrl && self.alt == alt && self.shift == shift;
        if self.win {
            return Some("klawisz Win steruje systemem, nie aplikacją");
        }
        match self.key {
            Tab if self.alt => Some("przełączanie zadań (Alt+Tab)"),
            Escape if self.alt || self.ctrl => {
                Some("przełączanie zadań / menu Start / Menedżer zadań")
            }
            Delete if self.ctrl && self.alt => Some("bezpieczna sekwencja (Ctrl+Alt+Del)"),
            Function(12) if only(true, false, true) => Some("kill-switch Jądra (Ctrl+Shift+F12)"),
            // Skróty globalne Alfy (powłoka): dyktowanie, czytanie, Szybkie pytanie (SR3-04).
            ChordKey::Space | ChordKey::Letter('D' | 'R') if only(true, true, false) => {
                Some("skrót globalny Alfy (dyktowanie, czytanie, Szybkie pytanie)")
            }
            _ => None,
        }
    }

    /// Kody wirtualne: modyfikatory (Ctrl, Alt, Shift, Win) i klawisz.
    pub fn vks(&self) -> (Vec<u16>, u16) {
        let mods = [
            (self.ctrl, VK_CONTROL),
            (self.alt, VK_MENU),
            (self.shift, VK_SHIFT),
            (self.win, VK_LWIN),
        ]
        .into_iter()
        .filter_map(|(on, vk)| on.then_some(vk))
        .collect();
        (mods, self.key.vk())
    }
}

impl fmt::Display for KeyChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, name) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
            (self.win, "Win+"),
        ] {
            if on {
                f.write_str(name)?;
            }
        }
        write!(f, "{:?}", self.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_chords() {
        let c = KeyChord::parse("ctrl + Shift+s").unwrap();
        assert!(c.ctrl && c.shift && !c.alt && c.key == ChordKey::Letter('S'));
        assert_eq!(c.vks(), (vec![VK_CONTROL, VK_SHIFT], 0x53));
        assert_eq!(KeyChord::parse("Alt+F4").unwrap().key.vk(), 0x73);
        assert_eq!(KeyChord::parse("Enter").unwrap().key, ChordKey::Enter);
        assert_eq!(
            KeyChord::parse("Page Down").unwrap().key,
            ChordKey::PageDown
        );
        assert_eq!(KeyChord::parse("ctrl+0").unwrap().key.vk(), 0x30);
        assert_eq!(KeyChord::parse("F24").unwrap().key.vk(), 0x87);
        for bad in ["", "Ctrl+", "Ctrl+Ctrl+S", "Hyper+S", "F25", "Ctrl+żółw"] {
            assert!(KeyChord::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(
            KeyChord::parse("Ctrl+Alt+End").unwrap().to_string(),
            "Ctrl+Alt+End"
        );
        assert!(is_extended_vk(ChordKey::Down.vk()) && !is_extended_vk(VK_RETURN));
    }

    #[test]
    fn system_chords_are_refused() {
        for s in [
            "Win+R",
            "Alt+Tab",
            "Alt+Shift+Tab",
            "Alt+Esc",
            "Ctrl+Esc",
            "Ctrl+Shift+Esc",
            "Ctrl+Alt+Del",
            "Ctrl+Shift+F12",
            "Win+L",
            "Ctrl+Alt+D",
            "Ctrl+Alt+Space",
        ] {
            assert!(KeyChord::parse(s).unwrap().system_scope().is_some(), "{s}");
        }
        for s in [
            "Ctrl+S",
            "Alt+F4",
            "Enter",
            "Ctrl+Shift+F11",
            "Ctrl+Alt+Shift+F12",
            "Ctrl+Alt+Shift+D",
            "Tab",
        ] {
            assert!(KeyChord::parse(s).unwrap().system_scope().is_none(), "{s}");
        }
    }
}
