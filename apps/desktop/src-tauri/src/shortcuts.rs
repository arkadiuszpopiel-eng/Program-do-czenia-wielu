//! Skróty globalne obsługiwane przez rdzeń (nie przez WebView): `Ctrl+Alt+Space` — Szybkie
//! pytanie, `Ctrl+Shift+F12` — STOP WSZYSTKIEGO (anulowanie wszystkich generacji + kill-switch
//! Brokera: tokeny, drzewa procesów, cisza audio). Konflikt rejestracji → `Toast` w UI.

use app_core::AppCore;
use app_core::dto::{AlfaEvent, LocalizedText, ToastKind};
use app_core::ports::KillOrigin;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::windows;

fn quick_ask() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space)
}

fn kill_switch() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F12)
}

/// Wtyczka z obsługą naciśnięć.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if shortcut.id() == quick_ask().id() {
                if let Err(e) = toggle(app) {
                    tracing::warn!(error = %e, "okno Szybkiego pytania niedostępne");
                }
            } else if shortcut.id() == kill_switch().id() {
                let core = app.state::<AppCore>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    let stopped = core.system_kill_all(KillOrigin::Hotkey).await;
                    tracing::warn!(zatrzymane = stopped, "STOP WSZYSTKIEGO (Ctrl+Shift+F12)");
                });
            }
        })
        .build()
}

fn toggle<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    match app.get_webview_window(windows::QUICK) {
        Some(window) if window.is_visible()? => window.hide(),
        Some(window) => {
            window.center()?;
            window.show()?;
            window.set_focus()
        }
        None => Ok(()),
    }
}

/// Rejestruje skróty; konflikt (np. zajęty przez PowerToys) → komunikat w UI.
pub fn register(app: &AppHandle, core: &AppCore) {
    for (shortcut, name) in [
        (quick_ask(), "Ctrl+Alt+Space"),
        (kill_switch(), "Ctrl+Shift+F12"),
    ] {
        if let Err(e) = app.global_shortcut().register(shortcut) {
            tracing::warn!(skrot = name, error = %e, "rejestracja skrótu globalnego nie powiodła się");
            core.emit_event(AlfaEvent::Toast {
                kind: ToastKind::Warning,
                message: LocalizedText::new(
                    format!("Skrót {name} jest zajęty przez inny program."),
                    format!("Shortcut {name} is taken by another app."),
                ),
            });
        }
    }
}
