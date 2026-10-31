//! Mysz nad oknem Sędziego Cieni: który przycisk jest pod kursorem (podświetlenie) i czy kursor
//! jest na nagłówku (przeciąganie okna jak za belkę tytułu — okno nie ma belki systemowej).

#![allow(unsafe_code)]

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetDlgCtrlID, GetParent, HTCAPTION, WindowFromPoint,
};

/// Identyfikator przycisku naszego okna `dialog`, jeśli `child` nim jest.
pub(super) fn control_of(dialog: HWND, child: HWND) -> Option<u16> {
    // SAFETY: zapytania o rodzica i identyfikator okna.
    unsafe {
        let parent = GetParent(child).ok()?;
        (parent == dialog).then(|| u16::try_from(GetDlgCtrlID(child)).ok())?
    }
}

/// Przycisk pod kursorem teraz (zegar podświetlenia).
pub(super) fn hover_now(dialog: HWND) -> Option<u16> {
    let mut pt = POINT::default();
    // SAFETY: położenie kursora do zmiennej lokalnej i okno pod nim.
    unsafe {
        GetCursorPos(&raw mut pt).ok()?;
        control_of(dialog, WindowFromPoint(pt))
    }
}

/// Nagłówek okna (bez przycisków — te są osobnymi oknami) przeciąga okno jak belka tytułu.
pub(super) fn caption_hit(hwnd: HWND, lparam: LPARAM, header: i32) -> Option<LRESULT> {
    // Współrzędne ekranu są 16-bitowe ze znakiem (monitory na lewo od głównego — ujemne).
    let signed = |v: isize| {
        let v = i32::try_from(v & 0xFFFF).unwrap_or(0);
        if v >= 0x8000 { v - 0x1_0000 } else { v }
    };
    let mut pt = POINT {
        x: signed(lparam.0),
        y: signed(lparam.0 >> 16),
    };
    // SAFETY: przeliczenie punktu ekranu na obszar klienta naszego okna.
    let _ = unsafe { ScreenToClient(hwnd, &raw mut pt) };
    (pt.y >= 0 && pt.y < header).then_some(LRESULT(HTCAPTION as isize))
}
