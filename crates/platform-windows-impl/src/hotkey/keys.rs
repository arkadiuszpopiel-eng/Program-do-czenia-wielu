//! Mapowanie `Hotkey` z kontraktu na modyfikatory `RegisterHotKey` i kody wirtualnych klawiszy.

use platform_contract::{Hotkey, Key, PlatformError};

/// `MOD_ALT`.
pub(crate) const MOD_ALT: u32 = 0x0001;
/// `MOD_CONTROL`.
pub(crate) const MOD_CONTROL: u32 = 0x0002;
/// `MOD_SHIFT`.
pub(crate) const MOD_SHIFT: u32 = 0x0004;
/// `MOD_WIN`.
pub(crate) const MOD_WIN: u32 = 0x0008;
/// `MOD_NOREPEAT` — przytrzymanie nie generuje powtórzeń (PTT).
pub(crate) const MOD_NOREPEAT: u32 = 0x4000;

const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_MENU: u32 = 0x12;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;
const VK_F1: u32 = 0x70;
const VK_LSHIFT: u32 = 0xA0;
const VK_RSHIFT: u32 = 0xA1;
const VK_LCONTROL: u32 = 0xA2;
const VK_RCONTROL: u32 = 0xA3;
const VK_LMENU: u32 = 0xA4;
const VK_RMENU: u32 = 0xA5;

/// Skrót w postaci natywnej.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeHotkey {
    /// Maska `MOD_*` (bez `MOD_NOREPEAT`).
    pub(crate) modifiers: u32,
    /// Kod wirtualnego klawisza.
    pub(crate) vk: u32,
}

/// Konwersja na postać natywną (klawisze spoza zakresu → `HotkeyRejected`).
pub(crate) fn to_native(hotkey: &Hotkey) -> Result<NativeHotkey, PlatformError> {
    let reject = |why: &str| PlatformError::HotkeyRejected(format!("{hotkey}: {why}"));
    let vk = match hotkey.key {
        Key::Letter(c) if c.is_ascii_alphabetic() => u32::from(c.to_ascii_uppercase()),
        Key::Letter(_) => return Err(reject("dozwolone tylko litery A–Z")),
        Key::Digit(d) if d <= 9 => 0x30 + u32::from(d),
        Key::Digit(_) => return Err(reject("cyfra spoza 0–9")),
        Key::Function(n) if (1..=24).contains(&n) => VK_F1 + u32::from(n) - 1,
        Key::Function(_) => return Err(reject("klawisz funkcyjny spoza F1–F24")),
        Key::Space => VK_SPACE,
        Key::Escape => VK_ESCAPE,
    };
    let m = hotkey.modifiers;
    let modifiers = [
        (m.alt, MOD_ALT),
        (m.ctrl, MOD_CONTROL),
        (m.shift, MOD_SHIFT),
        (m.win, MOD_WIN),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .fold(0, |acc, (_, bit)| acc | bit);
    Ok(NativeHotkey { modifiers, vk })
}

/// Czy puszczony klawisz `vk` jest jednym z wymaganych modyfikatorów (lewym, prawym lub ogólnym).
pub(crate) fn releases_modifier(vk: u32, modifiers: u32) -> bool {
    let groups: [(u32, &[u32]); 4] = [
        (MOD_CONTROL, &[VK_CONTROL, VK_LCONTROL, VK_RCONTROL]),
        (MOD_ALT, &[VK_MENU, VK_LMENU, VK_RMENU]),
        (MOD_SHIFT, &[VK_SHIFT, VK_LSHIFT, VK_RSHIFT]),
        (MOD_WIN, &[VK_LWIN, VK_RWIN]),
    ];
    groups
        .iter()
        .any(|(bit, vks)| modifiers & bit != 0 && vks.contains(&vk))
}

/// Klawisze do odpytania, czy kombinacja jest nadal wciśnięta (główny + ogólne modyfikatory;
/// Win: oba warianty — wystarczy jeden wciśnięty).
pub(crate) fn held_groups(native: &NativeHotkey) -> Vec<Vec<u32>> {
    let mut groups = vec![vec![native.vk]];
    for (bit, vks) in [
        (MOD_CONTROL, vec![VK_CONTROL]),
        (MOD_ALT, vec![VK_MENU]),
        (MOD_SHIFT, vec![VK_SHIFT]),
        (MOD_WIN, vec![VK_LWIN, VK_RWIN]),
    ] {
        if native.modifiers & bit != 0 {
            groups.push(vks);
        }
    }
    groups
}

/// Pomocniczo do testów i dokumentacji: modyfikatory z maski.
#[cfg(test)]
pub(crate) fn modifiers_of(mask: u32) -> platform_contract::Modifiers {
    platform_contract::Modifiers {
        ctrl: mask & MOD_CONTROL != 0,
        alt: mask & MOD_ALT != 0,
        shift: mask & MOD_SHIFT != 0,
        win: mask & MOD_WIN != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_contract::{KILL_SWITCH, Modifiers};

    #[test]
    fn maps_keys_and_modifiers() {
        let n = to_native(&KILL_SWITCH).unwrap();
        assert_eq!(n.vk, 0x7B);
        assert_eq!(n.modifiers, MOD_CONTROL | MOD_SHIFT);
        assert_eq!(modifiers_of(n.modifiers), KILL_SWITCH.modifiers);
        let ctrl_alt = Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::default()
        };
        let space = to_native(&Hotkey::new(ctrl_alt, Key::Space)).unwrap();
        assert_eq!((space.vk, space.modifiers), (0x20, MOD_CONTROL | MOD_ALT));
        assert_eq!(
            to_native(&Hotkey::new(ctrl_alt, Key::Letter('b')))
                .unwrap()
                .vk,
            0x42
        );
        assert_eq!(
            to_native(&Hotkey::new(ctrl_alt, Key::Digit(7))).unwrap().vk,
            0x37
        );
        assert_eq!(
            to_native(&Hotkey::new(ctrl_alt, Key::Function(24)))
                .unwrap()
                .vk,
            0x87
        );
        assert_eq!(
            to_native(&Hotkey::new(ctrl_alt, Key::Escape)).unwrap().vk,
            0x1B
        );
        for bad in [
            Key::Letter('ś'),
            Key::Digit(10),
            Key::Function(0),
            Key::Function(25),
        ] {
            assert!(matches!(
                to_native(&Hotkey::new(ctrl_alt, bad)),
                Err(PlatformError::HotkeyRejected(_))
            ));
        }
    }

    #[test]
    fn release_detection_helpers() {
        let mods = MOD_CONTROL | MOD_WIN;
        assert!(releases_modifier(VK_LCONTROL, mods));
        assert!(releases_modifier(VK_RWIN, mods));
        assert!(!releases_modifier(VK_LSHIFT, mods));
        assert!(!releases_modifier(0x41, mods));
        let groups = held_groups(&NativeHotkey {
            modifiers: mods,
            vk: 0x20,
        });
        assert_eq!(
            groups,
            vec![vec![0x20], vec![VK_CONTROL], vec![VK_LWIN, VK_RWIN]]
        );
    }
}
