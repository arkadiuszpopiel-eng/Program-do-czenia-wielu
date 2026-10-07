//! Procesy Jądra bezpieczeństwa (ADR 0003, PLAN §8.1–8.2, §8.6; `app-broker`): Broker poza
//! procesem Tauri (usługa `AlfaBroker` albo tryb przenośny `alfa-broker --console` z oknem
//! Broker-UI uruchamianym przez Brokera), `alfa-watchdog` z `Ctrl+Shift+F12` poza UI.
//!
//! Build produkcyjny (release) bez izolowanego Brokera działa w **bezpiecznym stanie** („brak”:
//! nic, co wymaga zgody, nie zostanie wykonane) — nigdy po cichu z Brokerem w procesie Tauri.
//! Broker w procesie zostaje dla buildu deweloperskiego (debug) i Linuksa/CI, jawnie oznaczony
//! w UI. Zatwierdzanie nigdy nie odbywa się w WebView.

use app_core::AppCore;
use app_core::app_broker::RemoteKernel;
use app_core::app_broker::kernel::{KernelProcesses, KernelSetup, KernelStart};
use app_core::dto::AlfaEvent;
use app_core::ports::KillOrigin;
use tauri::AppHandle;

use crate::shortcuts;

/// Procesy Jądra żyjące razem z aplikacją (stan zarządzany Tauri).
pub struct Kernel(pub Option<KernelProcesses>);

/// Wybór Brokera dla `AppOptions::kernel` (przed budową rdzenia).
pub fn start() -> (Option<RemoteKernel>, Kernel) {
    if !cfg!(windows) {
        // Linux/CI: Broker w procesie (testy, tryb deweloperski).
        return (None, Kernel(None));
    }
    let started = KernelSetup::system().map(KernelProcesses::start);
    match started {
        Ok(KernelStart::Remote(procs)) => (Some(procs.remote()), Kernel(Some(procs))),
        Ok(KernelStart::InProcess(why)) | Err(why) => (fallback(&why), Kernel(None)),
    }
}

fn fallback(why: &str) -> Option<RemoteKernel> {
    if cfg!(debug_assertions) {
        tracing::warn!(
            powod = why,
            "Broker w procesie Tauri — wyłącznie build deweloperski"
        );
        None
    } else {
        tracing::error!(
            powod = why,
            "brak izolowanego Brokera — bezpieczny stan (fail-closed)"
        );
        Some(RemoteKernel::unavailable(why))
    }
}

/// Po zbudowaniu rdzenia: stan Brokera → zdarzenie `BrokerStatus` (baner, Ustawienia), skrót
/// `Ctrl+Shift+F12` w aplikacji tylko awaryjnie (gdy watchdog nie działa), kill-switch
/// watchdoga → „STOP WSZYSTKIEGO” w aplikacji (generacje, przebiegi, mowa, drzewa narzędzi).
pub fn watch(app: &AppHandle, core: &AppCore, kernel: &Kernel) {
    let Some(procs) = kernel.0.as_ref() else {
        shortcuts::set_kill_switch(app, core, true);
        return;
    };
    shortcuts::set_kill_switch(app, core, !procs.watchdog_active());
    let mut status = procs.subscribe();
    let (app_s, core_s) = (app.clone(), core.clone());
    tauri::async_runtime::spawn(async move {
        while status.changed().await.is_ok() {
            let view = status.borrow_and_update().clone();
            shortcuts::set_kill_switch(&app_s, &core_s, !view.watchdog);
            core_s.emit_event(AlfaEvent::BrokerStatus { status: view });
        }
    });
    let mut kills = procs.kills();
    let core_k = core.clone();
    tauri::async_runtime::spawn(async move {
        while kills.changed().await.is_ok() {
            let stopped = core_k.system_kill_all(KillOrigin::Hotkey).await;
            tracing::warn!(
                zatrzymane = stopped,
                "STOP WSZYSTKIEGO (watchdog, Ctrl+Shift+F12)"
            );
        }
    });
}
