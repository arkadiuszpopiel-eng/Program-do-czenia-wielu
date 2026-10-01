//! Zasobnik (PLAN §14.8): pokaż, nowa rozmowa, Szybkie pytanie, głos wł./wył., nie przeszkadzać,
//! STOP WSZYSTKIEGO, wyjście. Klik lewym przyciskiem pokazuje okno główne.

use std::sync::atomic::Ordering;

use app_core::AppCore;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::pump::DoNotDisturb;
use crate::{shell, windows};

/// Pozycje menu z przełącznikami.
struct TrayToggles {
    voice: CheckMenuItem<Wry>,
    dnd: CheckMenuItem<Wry>,
}

/// Buduje ikonę i menu zasobnika.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Pokaż Alfę", true, None::<&str>)?;
    let new = MenuItem::with_id(app, "new", "Nowa rozmowa", true, None::<&str>)?;
    let quick = MenuItem::with_id(
        app,
        "quick",
        "Szybkie pytanie",
        true,
        Some("Ctrl+Alt+Space"),
    )?;
    let voice = CheckMenuItem::with_id(app, "voice", "Głos", true, false, None::<&str>)?;
    let dnd = CheckMenuItem::with_id(app, "dnd", "Nie przeszkadzać", true, false, None::<&str>)?;
    let stop = MenuItem::with_id(
        app,
        "stop",
        "STOP WSZYSTKIEGO",
        true,
        Some("Ctrl+Shift+F12"),
    )?;
    let quit = MenuItem::with_id(app, "quit", "Wyjście", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &new,
            &quick,
            &PredefinedMenuItem::separator(app)?,
            &voice,
            &dnd,
            &PredefinedMenuItem::separator(app)?,
            &stop,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    app.manage(TrayToggles { voice, dnd });
    let mut builder = TrayIconBuilder::with_id("alfa")
        .tooltip("Alfa")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = windows::show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    let core = app.state::<AppCore>().inner().clone();
    let id = event.id().as_ref().to_owned();
    let result = match id.as_str() {
        "show" => windows::show_main(app),
        "quick" => windows::toggle_quick(app),
        "quit" => {
            app.exit(0);
            Ok(())
        }
        "dnd" => {
            let on = app.state::<TrayToggles>().dnd.is_checked().unwrap_or(false);
            app.state::<DoNotDisturb>().0.store(on, Ordering::Relaxed);
            Ok(())
        }
        _ => Ok(()),
    };
    if let Err(e) = result {
        tracing::warn!(pozycja = id, error = %e, "akcja zasobnika nie powiodła się");
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match id.as_str() {
            "new" => {
                if let Err(e) = shell::new_chat(&core, None).await {
                    tracing::warn!(error = %e, "nowa rozmowa z zasobnika nie powiodła się");
                }
                let _ = windows::show_main(&app);
            }
            "stop" => {
                let stopped = core.system_kill_all().await;
                tracing::warn!(zatrzymane = stopped, "STOP WSZYSTKIEGO z zasobnika");
            }
            "voice" => {
                let toggles = app.state::<TrayToggles>();
                let on = toggles.voice.is_checked().unwrap_or(false);
                if let Err(e) = core.voice_set_mic_enabled(on).await {
                    let _ = toggles.voice.set_checked(false);
                    tracing::warn!(error = %e, "przełączenie głosu nie powiodło się");
                }
            }
            _ => {}
        }
    });
}
