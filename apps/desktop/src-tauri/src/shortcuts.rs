//! Skróty globalne obsługiwane przez rdzeń (nie przez WebView): `Ctrl+Alt+Space` — Szybkie
//! pytanie, `Ctrl+Shift+F12` — STOP WSZYSTKIEGO (anulowanie wszystkich generacji + kill-switch
//! Brokera: tokeny, drzewa procesów, cisza audio), F5: `Ctrl+Alt+D` — dyktowanie wł./wył. do okna
//! na pierwszym planie, `Ctrl+Alt+R` — czytaj zaznaczenie (D i R nie są literami polskimi —
//! reguła AltGr). Konflikt rejestracji → `Toast` w UI.
//!
//! Kill-switch należy do `alfa-watchdog` (poza UI, PLAN §8.6): gdy watchdog działa, aplikacja
//! **nie** rejestruje `Ctrl+Shift+F12` (dostaje od watchdoga komunikat i zatrzymuje generacje —
//! `kernel::watch`); rejestruje go wyłącznie awaryjnie, gdy watchdoga brak albo się zakończył
//! (baner w UI) — [`set_kill_switch`].

use app_core::AppCore;
use app_core::dto::{AlfaEvent, DictationAction, LocalizedText, ReadAction, ReadSource, ToastKind};
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

fn dictation() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyD)
}

fn read_selection() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyR)
}

/// Głos rozszerzony ze skrótu: błąd (odmowa w polu hasła, brak modeli) → komunikat w UI.
fn voice_shortcut<R: Runtime>(app: &AppHandle<R>, read: bool) {
    let core = app.state::<AppCore>().inner().clone();
    tauri::async_runtime::spawn(async move {
        let result = if read {
            let source = ReadSource::Selection;
            core.voice_read(ReadAction::Start { source }).await
        } else {
            core.voice_dictation(DictationAction::Toggle).await
        };
        if let Err(e) = result {
            core.emit_event(AlfaEvent::Toast {
                kind: ToastKind::Warning,
                message: LocalizedText::new(e.message, e.code.english()),
            });
        }
    });
}

/// Wtyczka z obsługą naciśnięć.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                on_pressed(app, shortcut);
            }
        })
        .build()
}

/// Naciśnięcie skrótu — wspólne dla systemu (wtyczka) i testu prawdziwej aplikacji
/// ([`e2e_global_shortcut`]).
fn on_pressed<R: Runtime>(app: &AppHandle<R>, shortcut: &Shortcut) {
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
    } else if shortcut.id() == dictation().id() {
        voice_shortcut(app, false);
    } else if shortcut.id() == read_selection().id() {
        voice_shortcut(app, true);
    }
}

/// Skróty globalne z nazwami jak w komunikatach i na stronie Skróty.
fn named() -> [(Shortcut, &'static str); 4] {
    [
        (quick_ask(), "Ctrl+Alt+Space"),
        (kill_switch(), "Ctrl+Shift+F12"),
        (dictation(), "Ctrl+Alt+D"),
        (read_selection(), "Ctrl+Alt+R"),
    ]
}

/// `e2e_global_shortcut` — wyłącznie budowa testowa (cecha `e2e`): naciśnięcie skrótu globalnego
/// `keys` tą samą obsługą co naciśnięcie w systemie, bez wstrzykiwania klawiszy w sesję właściciela
/// (test nie może wejść mu w pisanie). Dyktowanie i czytanie zaznaczenia działają na oknie na
/// pierwszym planie — najpierw fokus na oknie głównym instancji testowej; bez fokusu odmowa, zanim
/// cokolwiek sięgnie do innego programu. Wynik: czy ta instancja ma skrót zarejestrowany w systemie.
/// Poza budową testową — odmowa.
#[tauri::command]
pub async fn e2e_global_shortcut(app: AppHandle, keys: String) -> Result<bool, String> {
    if !cfg!(feature = "e2e") {
        return Err("Polecenie tylko dla budowy testowej.".into());
    }
    let (shortcut, _) = named()
        .into_iter()
        .find(|(_, name)| *name == keys)
        .ok_or_else(|| format!("Nieznany skrót globalny: {keys}"))?;
    if shortcut.id() == dictation().id() || shortcut.id() == read_selection().id() {
        focus_main(&app).await?;
    }
    let registered = app.global_shortcut().is_registered(shortcut);
    on_pressed(&app, &shortcut);
    Ok(registered)
}

/// Okno główne na pierwszy plan (do 3 s); bez fokusu — błąd.
async fn focus_main(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(windows::MAIN)
        .ok_or("Brak okna głównego.")?;
    if crate::cdp::hidden_instance() {
        // Fokus zabrałby klawiaturę właścicielowi; ten skrót sprawdza bieg z oknami na ekranie.
        return Err("Instancja testowa pracuje poza ekranem — dyktowanie i czytanie zaznaczenia                     sprawdza bieg z ALFA_E2E_VISIBLE=1."
            .into());
    }
    let shown = window.show().and_then(|()| window.set_focus());
    shown.map_err(|e| format!("Fokus okna głównego: {e}"))?;
    for _ in 0..30 {
        if window.is_focused().unwrap_or(false) {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err("Okno instancji testowej nie dostało fokusu — skrót nie naciśnięty.".into())
}

fn toggle<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    match app.get_webview_window(windows::QUICK) {
        Some(window) if window.is_visible()? => window.hide(),
        Some(window) => {
            window.center()?;
            windows::present(&window)
        }
        None => Ok(()),
    }
}

fn register_one(app: &AppHandle, core: &AppCore, shortcut: Shortcut, name: &str) {
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

/// Rejestruje skróty aplikacji (kill-switch osobno — [`set_kill_switch`]); konflikt (np. zajęty
/// przez PowerToys) → komunikat w UI.
pub fn register(app: &AppHandle, core: &AppCore) {
    for (shortcut, name) in named() {
        if shortcut.id() != kill_switch().id() {
            register_one(app, core, shortcut, name);
        }
    }
}

/// `Ctrl+Shift+F12` w aplikacji: `true` — awaryjnie (watchdoga brak), `false` — obsługuje go
/// `alfa-watchdog` (skrót zwolniony, żeby nie był zarejestrowany dwa razy).
pub fn set_kill_switch(app: &AppHandle, core: &AppCore, in_app: bool) {
    let registered = app.global_shortcut().is_registered(kill_switch());
    if in_app && !registered {
        tracing::warn!("Ctrl+Shift+F12 obsługuje aplikacja (awaryjnie — watchdog nie działa)");
        register_one(app, core, kill_switch(), "Ctrl+Shift+F12");
    } else if !in_app
        && registered
        && let Err(e) = app.global_shortcut().unregister(kill_switch())
    {
        tracing::warn!(error = %e, "zwolnienie Ctrl+Shift+F12 dla watchdoga nie powiodło się");
    }
}
