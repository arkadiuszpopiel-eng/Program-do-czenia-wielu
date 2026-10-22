//! Dziennik rozruchu w programie (`app-logs`): komenda `boot_report` dla UI (Ustawienia → Zdrowie
//! systemu, sygnał „rozruch OK / z uwagami / z problemem”), zamknięcie okna rozruchu po starcie i
//! znacznik nieudanego startu. Kopia właściciela nie ma okna konsoli — błąd startu, po którym program
//! się zamyka, zostawia `rozruch-nieudany.txt` w katalogu dziennika, a następny start zgłasza go
//! jako błąd rozruchu.

use std::path::PathBuf;
use std::time::Duration;

use app_core::AppPaths;
use app_logs::{BootReport, BootVerdict, LogHandle};

/// Ile po złożeniu rdzenia i okien zbierać linie rozruchu (procesy Jądra, watchdog, mowa).
pub const WINDOW_AFTER_SETUP: Duration = Duration::from_secs(10);

/// Plik znacznika nieudanego startu.
const FAILED_MARKER: &str = "rozruch-nieudany.txt";

/// Uchwyt dziennika w stanie Tauri (`None` — dziennik nie wystartował).
pub struct BootLog(pub Option<LogHandle>);

fn marker() -> Option<PathBuf> {
    AppPaths::from_env()
        .ok()
        .map(|p| p.logs().join(FAILED_MARKER))
}

/// Na starcie: znacznik poprzedniego nieudanego startu → błąd rozruchu (i usunięcie znacznika).
pub fn report_previous_failure() {
    let Some(path) = marker() else { return };
    if let Ok(text) = std::fs::read_to_string(&path) {
        let _ = std::fs::remove_file(&path);
        tracing::error!(
            powod = %text.trim(),
            "poprzedni start Alfy się nie powiódł (program zamknął się przed oknem)"
        );
    }
}

/// Błąd, po którym program się zamyka: znacznik dla następnego startu.
pub fn mark_failed(why: &str) {
    if let Some(path) = marker() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, format!("{why}\n"));
    }
}

/// Po starcie: zamyka okno rozruchu po [`WINDOW_AFTER_SETUP`] (wątek, bez blokowania UI).
pub fn finish_later(handle: Option<LogHandle>) {
    let Some(handle) = handle else { return };
    handle.boot_ready();
    let spawned = std::thread::Builder::new()
        .name("alfa-boot".into())
        .spawn(move || {
            std::thread::sleep(WINDOW_AFTER_SETUP);
            let report = handle.boot_report();
            tracing::info!(
                ms = report.elapsed_ms,
                ostrzezenia = report.warnings,
                bledy = report.errors,
                "rozruch zakończony"
            );
            handle.boot_finish();
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "okno rozruchu bez zamknięcia (wątek nie wystartował)");
    }
}

/// Raport bez dziennika (subskrybent nie wystartował).
fn unavailable() -> BootReport {
    BootReport {
        started_at: String::new(),
        finished: true,
        elapsed_ms: 0,
        verdict: BootVerdict::Warnings,
        warnings: 1,
        errors: 0,
        lines: vec![app_logs::BootLine {
            level: "WARN",
            text: "Dziennik diagnostyczny nie wystartował — brak linii rozruchu.".into(),
        }],
        dropped: 0,
        log_file: None,
    }
}

/// `boot_report` — dziennik rozruchu (tylko odczyt, bez treści rozmów: linie dziennika po redakcji).
#[tauri::command]
pub fn boot_report(state: tauri::State<'_, BootLog>) -> BootReport {
    state
        .0
        .as_ref()
        .map_or_else(unavailable, LogHandle::boot_report)
}
