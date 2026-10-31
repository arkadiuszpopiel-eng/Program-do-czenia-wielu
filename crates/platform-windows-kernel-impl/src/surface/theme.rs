//! Kolory okna Sędziego Cieni — wartości tokenów `packages/ui-kit/src/tokens.css` (`--alfa-color-*`)
//! w motywie jasnym i ciemnym; motyw według ustawienia Windows (`AppsUseLightTheme`).

#![allow(unsafe_code)]

use std::ffi::c_void;

use platform_contract::SurfaceTone;
use windows::Win32::Foundation::COLORREF;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::w;

const fn hex(v: u32) -> COLORREF {
    // 0xRRGGBB → COLORREF (0x00BBGGRR).
    COLORREF(((v >> 16) & 0xFF) | (v & 0xFF00) | ((v & 0xFF) << 16))
}

/// Kolory motywu — wartości tokenów `packages/ui-kit/src/tokens.css`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Palette {
    pub dark: bool,
    pub bg: COLORREF,
    pub surface: COLORREF,
    pub surface2: COLORREF,
    pub border: COLORREF,
    pub text: COLORREF,
    pub muted: COLORREF,
    pub subtle: COLORREF,
    pub focus: COLORREF,
    pub success: COLORREF,
    pub warning: COLORREF,
    pub error: COLORREF,
}

const LIGHT: Palette = Palette {
    dark: false,
    bg: hex(0xFAFAFB),
    surface: hex(0xFFFFFF),
    surface2: hex(0xF1F2F5),
    border: hex(0xE2E4EA),
    text: hex(0x1A1C22),
    muted: hex(0x5B6070),
    subtle: hex(0x626878),
    focus: hex(0x2F5BFF),
    success: hex(0x127436),
    warning: hex(0x8A5A00),
    error: hex(0xB91C1C),
};

const DARK: Palette = Palette {
    dark: true,
    bg: hex(0x131417),
    surface: hex(0x1B1C21),
    surface2: hex(0x23252C),
    border: hex(0x2E313A),
    text: hex(0xEDEEF2),
    muted: hex(0xA7ACBB),
    subtle: hex(0x8A8F9E),
    focus: hex(0x8FB0FF),
    success: hex(0x4ADE80),
    warning: hex(0xFBBF24),
    error: hex(0xF87171),
};

impl Palette {
    /// Kolor ryzyka (`--alfa-color-risk-*`).
    pub(super) fn risk(&self, tone: SurfaceTone) -> COLORREF {
        match tone {
            SurfaceTone::Neutral => self.muted,
            SurfaceTone::Low => self.success,
            SurfaceTone::Medium => self.warning,
            SurfaceTone::High | SurfaceTone::Critical => self.error,
        }
    }
}

/// Motyw jasny albo ciemny (podgląd okna w pliku).
pub(super) fn palette_of(dark: bool) -> Palette {
    if dark { DARK } else { LIGHT }
}

/// Motyw aplikacji według ustawień Windows (`AppsUseLightTheme`; błąd odczytu — jasny).
pub(super) fn palette() -> Palette {
    let mut value: u32 = 1;
    let mut size = u32::try_from(size_of::<u32>()).unwrap_or(4);
    // SAFETY: odczyt wartości DWORD do zmiennej lokalnej.
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast::<c_void>()),
            Some(&raw mut size),
        )
    };
    if read.is_ok() && value == 0 {
        DARK
    } else {
        LIGHT
    }
}
