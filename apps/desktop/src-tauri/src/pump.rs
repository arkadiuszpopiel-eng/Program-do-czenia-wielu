//! Pompa zdarzeń: paczki `AlfaEvent[]` z `AppCore` (batch co klatkę) → `alfa://events` do okien;
//! przy ukrytym oknie głównym — natywne powiadomienia (bez akcji zatwierdzania, SPEC notify).

use std::sync::atomic::{AtomicBool, Ordering};

use app_core::AppCore;
use app_core::notify::native_notice;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::broadcast::error::RecvError;

use crate::windows;

/// Jedyny kanał zdarzeń rdzeń → UI.
pub const EVENTS_CHANNEL: &str = "alfa://events";

/// Tryb „nie przeszkadzać" (zasobnik): wycisza powiadomienia natywne.
#[derive(Default)]
pub struct DoNotDisturb(pub AtomicBool);

/// Uruchamia pompę.
pub fn spawn(app: AppHandle, core: AppCore) {
    app.manage(DoNotDisturb::default());
    let mut rx = core.subscribe_events();
    tauri::async_runtime::spawn(async move {
        loop {
            let batch = match rx.recv().await {
                Ok(batch) => batch,
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(pominiete = skipped, "UI nie nadąża za paczkami zdarzeń");
                    continue;
                }
                Err(RecvError::Closed) => break,
            };
            if let Err(e) = app.emit(EVENTS_CHANNEL, batch.as_slice()) {
                tracing::error!(error = %e, "emit alfa://events nie powiódł się");
            }
            let quiet = app.state::<DoNotDisturb>().0.load(Ordering::Relaxed);
            if quiet || windows::main_visible(&app) {
                continue;
            }
            for notice in batch.iter().filter_map(native_notice) {
                let shown = app
                    .notification()
                    .builder()
                    .title(notice.title)
                    .body(notice.body)
                    .show();
                if let Err(e) = shown {
                    tracing::warn!(error = %e, "powiadomienie natywne nie powiodło się");
                }
            }
        }
    });
}
