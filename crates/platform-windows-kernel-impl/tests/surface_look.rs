//! Podgląd wyglądu okna Broker-UI na prawdziwym pulpicie (self-hosted, `#[ignore]`): okno wysokiego
//! ryzyka przez `ALFA_LOOK_MS` ms (domyślnie 6 s) — do zrzutu ekranu przy przeglądzie wyglądu.
//! Niczego nie klika; okno znika samo.

#![cfg(windows)]
#![allow(clippy::unwrap_used, unsafe_code)]

use std::time::Duration;

use platform_contract::{ApprovalSurfacePort, SurfaceButton, SurfaceTone, SurfaceView};
use platform_windows_kernel_impl::WinApprovalSurface;
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect};
use windows::core::w;

#[test]
#[ignore = "self-hosted: wymaga interaktywnego pulpitu (podgląd wyglądu)"]
fn broker_window_look_preview() {
    let s = WinApprovalSurface::new();
    let detail = |k: &str, v: &str| (k.to_owned(), v.to_owned());
    let view = SurfaceView {
        title: "Zmiana poziomu autonomii".into(),
        badge: "⚠ Ryzyko: wysokie".into(),
        tone: SurfaceTone::High,
        details: vec![
            detail("Kto", "Jądro Alfy"),
            detail("Dotyczy", "cała aplikacja"),
            detail("Zmiana", "L3 „Bardzo wysoka” → L4 „Maks”"),
            detail("Ryzyko", "wysokie — tak (dziennik cofania)"),
            detail("Źródło polecenia", "Ty (tekst)"),
            detail("Wygasa za", "9 min 59 s"),
        ],
        status: "Tab — następny przycisk, Spacja — naciśnij. Enter niczego nie zatwierdza.".into(),
        buttons: vec![
            SurfaceButton {
                id: 100,
                label: "Odmów (Esc)".into(),
            },
            SurfaceButton {
                id: 101,
                label: "Zezwól tylko teraz".into(),
            },
        ],
        initial_focus: 100,
        take_focus: true,
    };
    s.present(&view).unwrap();
    std::thread::sleep(Duration::from_millis(700));
    // Położenie okna (ekran) dla skryptu zrzutu — `ALFA_LOOK_RECT` = plik docelowy.
    if let Ok(path) = std::env::var("ALFA_LOOK_RECT") {
        let mut r = RECT::default();
        // SAFETY: zapytania o okno klasy Broker-UI tego procesu (tylko odczyt).
        unsafe {
            if let Ok(h) = FindWindowW(w!("AlfaBrokerUi"), None) {
                let _ = GetWindowRect(h, &raw mut r);
            }
        }
        let text = format!(
            "{} {} {} {}",
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top
        );
        std::fs::write(path, text).unwrap();
    }
    let ms = std::env::var("ALFA_LOOK_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6_000);
    std::thread::sleep(Duration::from_millis(ms));
    s.dismiss().unwrap();
}
