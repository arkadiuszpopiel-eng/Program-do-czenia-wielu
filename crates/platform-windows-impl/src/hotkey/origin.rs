//! Pochodzenie naciśnięć skrótów globalnych (przegląd bezpieczeństwa #2, P2-04): hook
//! `WH_KEYBOARD_LL` zapisuje dla każdego wciśniętego klawisza, czy zdarzenie było wstrzyknięte
//! (`LLKHF_INJECTED`, `LLKHF_LOWER_IL_INJECTED`) i kiedy (`KBDLLHOOKSTRUCT.time`). Gdy przychodzi
//! `WM_HOTKEY`, pochodzenie kombinacji to: wstrzyknięte, jeśli wstrzyknięto klawisz główny albo
//! którykolwiek modyfikator; fizyczne, jeśli hook widział wszystkie fizycznie (klawisz główny
//! w oknie [`HOOK_WINDOW_MS`] przed komunikatem); inaczej nieznane. Decyzję podejmuje
//! `HotkeyPressOrigin::admits` z kontraktu (kill-switch zawsze).
//!
//! Logika bez FFI — testowana na każdej platformie; hook i `WM_HOTKEY` są w `thread.rs`.

use std::collections::BTreeMap;

use platform_contract::{HOOK_WINDOW_MS, HookOrigin, HotkeyPressOrigin};

use super::keys::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, NativeHotkey};

/// Najwięcej pamiętanych klawiszy (każdy VK raz; zakres VK to 0–255).
const MAX_KEYS: usize = 256;

/// Grupy kodów klawiszy kombinacji (główny, potem modyfikatory w wariantach ogólnym, lewym
/// i prawym — hook LL zgłasza warianty lewy/prawy).
fn groups(native: &NativeHotkey) -> Vec<Vec<u32>> {
    let mut out = vec![vec![native.vk]];
    for (bit, vks) in [
        (MOD_CONTROL, vec![0x11, 0xA2, 0xA3]),
        (MOD_ALT, vec![0x12, 0xA4, 0xA5]),
        (MOD_SHIFT, vec![0x10, 0xA0, 0xA1]),
        (MOD_WIN, vec![0x5B, 0x5C]),
    ] {
        if native.modifiers & bit != 0 {
            out.push(vks);
        }
    }
    out
}

/// Klawisze wciśnięte według hooka (puszczenie usuwa wpis).
#[derive(Debug, Default)]
pub(crate) struct KeyOrigins {
    /// VK → (wstrzyknięte, czas wciśnięcia z hooka w ms).
    downs: BTreeMap<u32, (bool, u32)>,
}

impl KeyOrigins {
    /// Zapis wciśnięcia (z `KBDLLHOOKSTRUCT.flags` i `.time`; powtórzenia nadpisują).
    pub(crate) fn record_down(&mut self, vk: u32, flags: u32, time: u32) {
        if self.downs.len() >= MAX_KEYS && !self.downs.contains_key(&vk) {
            return;
        }
        let injected = HookOrigin::from_keyboard_flags(flags).is_injected();
        self.downs.insert(vk, (injected, time));
    }

    /// Zapis puszczenia.
    pub(crate) fn record_up(&mut self, vk: u32) {
        self.downs.remove(&vk);
    }

    /// Pochodzenie kombinacji dla `WM_HOTKEY` z czasem komunikatu `message_time` (ms, zegar
    /// `GetTickCount` — ta sama domena co `KBDLLHOOKSTRUCT.time`, z zawijaniem). Wstrzyknięty
    /// którykolwiek wciśnięty klawisz grupy = wstrzyknięte (fail-closed).
    pub(crate) fn origin(&self, native: &NativeHotkey, message_time: u32) -> HotkeyPressOrigin {
        let mut unknown = false;
        for (i, group) in groups(native).iter().enumerate() {
            let held: Vec<(bool, u32)> = group
                .iter()
                .filter_map(|vk| self.downs.get(vk).copied())
                .collect();
            if held.iter().any(|(injected, _)| *injected) {
                return HotkeyPressOrigin::Injected;
            }
            let freshest = held
                .iter()
                .map(|(_, t)| u64::from(message_time.wrapping_sub(*t)))
                .min();
            match freshest {
                None => unknown = true,
                Some(age) if i == 0 && age > HOOK_WINDOW_MS => unknown = true,
                Some(_) => {}
            }
        }
        if unknown {
            HotkeyPressOrigin::Unknown
        } else {
            HotkeyPressOrigin::Physical
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_contract::{LLKHF_INJECTED, LLKHF_LOWER_IL_INJECTED};

    const SPACE: u32 = 0x20;
    const LCTRL: u32 = 0xA2;
    const LALT: u32 = 0xA4;

    fn ctrl_alt_space() -> NativeHotkey {
        NativeHotkey {
            modifiers: MOD_CONTROL | MOD_ALT,
            vk: SPACE,
        }
    }

    #[test]
    fn physical_injected_and_unknown_presses() {
        let hk = ctrl_alt_space();
        let mut k = KeyOrigins::default();
        assert_eq!(
            k.origin(&hk, 100),
            HotkeyPressOrigin::Unknown,
            "hook nic nie widział"
        );
        k.record_down(LCTRL, 0, 10);
        k.record_down(LALT, 0, 20);
        k.record_down(SPACE, 0, 95);
        assert_eq!(k.origin(&hk, 100), HotkeyPressOrigin::Physical);
        // Klawisz główny sprzed okna obserwacji — nie wiadomo, czy to ta kombinacja.
        assert_eq!(k.origin(&hk, 95 + 5_000), HotkeyPressOrigin::Unknown);
        // Wstrzyknięta spacja przy fizycznych modyfikatorach.
        k.record_down(SPACE, LLKHF_INJECTED, 200);
        assert_eq!(k.origin(&hk, 201), HotkeyPressOrigin::Injected);
        // Po puszczeniu i fizycznym wciśnięciu — znów fizyczne.
        k.record_up(SPACE);
        k.record_down(SPACE, 0, 300);
        assert_eq!(k.origin(&hk, 301), HotkeyPressOrigin::Physical);
        // Wstrzyknięty prawy Ctrl przy fizycznym lewym — wstrzyknięte.
        k.record_down(0xA3, LLKHF_INJECTED, 310);
        assert_eq!(k.origin(&hk, 311), HotkeyPressOrigin::Injected);
        // Wstrzyknięty modyfikator (także z niższej integralności).
        let mut k = KeyOrigins::default();
        k.record_down(LCTRL, LLKHF_LOWER_IL_INJECTED, 10);
        k.record_down(LALT, 0, 20);
        k.record_down(SPACE, 0, 30);
        assert_eq!(k.origin(&hk, 31), HotkeyPressOrigin::Injected);
        // Zawinięcie licznika czasu.
        let mut k = KeyOrigins::default();
        k.record_down(0x11, 0, u32::MAX - 5);
        k.record_down(0x12, 0, u32::MAX - 4);
        k.record_down(SPACE, 0, u32::MAX - 1);
        assert_eq!(k.origin(&hk, 3), HotkeyPressOrigin::Physical);
    }
}
