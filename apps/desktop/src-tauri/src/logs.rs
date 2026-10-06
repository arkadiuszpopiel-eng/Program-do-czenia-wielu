//! Dziennik diagnostyczny powłoki (`app-logs`, PLAN §13): plik
//! `%LOCALAPPDATA%\Alfa\logs\alfa.<RRRR-MM-DD>.<NNN>.log`, instalowany na początku `run()` —
//! przed procesami Jądra i rdzeniem, więc ich błędy startu też trafiają do pliku. Poziom:
//! `ALFA_LOG`, a po zbudowaniu rdzenia `[logs] level` z konfiguracji; retencja
//! `[logs] file_days` (domyślnie 7 dni). W buildzie debug także stderr (`cargo tauri dev`).

use app_core::dto::SettingValue;
use app_core::{AppCore, AppPaths};
use app_logs::{LogConfig, LogHandle};

/// Instaluje subskrybenta `tracing` (`None`, gdy się nie da — aplikacja działa dalej bez pliku).
pub fn start() -> Option<LogHandle> {
    let dir = AppPaths::from_env().ok().map(|paths| paths.logs());
    match app_logs::install(LogConfig::for_process(app_logs::process::DESKTOP, dir)) {
        Ok(handle) => Some(handle),
        Err(e) => {
            eprintln!("alfa-desktop: {e}");
            None
        }
    }
}

/// Poziom i retencja z konfiguracji (`ALFA_LOG` ma pierwszeństwo przed poziomem).
pub fn apply_settings(handle: &LogHandle, core: &AppCore) {
    let level = tauri::async_runtime::block_on(core.setting(app_logs::LEVEL_KEY));
    let days = tauri::async_runtime::block_on(core.setting(app_logs::RETENTION_KEY));
    let level = match &level {
        Some(SettingValue::Text(text)) => Some(text.as_str()),
        _ => None,
    };
    let days = match &days {
        Some(SettingValue::Number(n)) => n.as_u64(),
        _ => None,
    };
    for problem in handle.apply_settings(level, days) {
        tracing::warn!(problem = %problem, "ustawienie dziennika pominięte");
    }
    let dir = handle.dir().map(|d| d.display().to_string());
    tracing::info!(
        katalog = dir.as_deref().unwrap_or("brak"),
        "dziennik diagnostyczny aktywny"
    );
}
