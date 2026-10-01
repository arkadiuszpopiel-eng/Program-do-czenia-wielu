//! Okna (COMMANDS.md → „Okna"): `main` 1200×800 (min. 400×500), `quick` 640 px bez dekoracji
//! (ukryte), `pill` 220×48 zawsze na wierzchu (ukryte). Wszystkie na wspólnym, stałym folderze
//! danych WebView2 poza katalogiem wersji (ADR 0007) — jeden proces przeglądarki (§14.2).
//! Zamknięcie okna głównego = ukrycie; po N minutach (`general.destroy_webview_after`) WebView
//! jest niszczony, a ponowne otwarcie tworzy go od nowa.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use app_core::AppCore;
use app_core::dto::SettingValue;
use tauri::{
    AppHandle, CloseRequestApi, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window,
};

/// Okno główne.
pub const MAIN: &str = "main";
/// Szybkie pytanie.
pub const QUICK: &str = "quick";
/// Pigułka głosowa.
pub const PILL: &str = "pill";

/// Stan okien zarządzany przez Tauri.
pub struct WindowState {
    data_dir: PathBuf,
    destroy_timer: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl WindowState {
    /// Stan z folderem danych WebView2.
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            destroy_timer: Mutex::new(None),
        }
    }

    fn cancel_timer(&self) {
        if let Ok(mut timer) = self.destroy_timer.lock()
            && let Some(task) = timer.take()
        {
            task.abort();
        }
    }
}

fn builder<'a>(
    app: &'a AppHandle,
    label: &str,
    page: &str,
    data_dir: PathBuf,
) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(PathBuf::from(page)))
        .data_directory(data_dir);
    // Port CDP tylko w buildzie testowym (Playwright); w produkcji zamknięty (PLAN §8.2).
    #[cfg(feature = "e2e")]
    let builder = builder.additional_browser_args(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port=9222",
    );
    builder
}

fn data_dir(app: &AppHandle) -> PathBuf {
    app.state::<WindowState>().data_dir.clone()
}

/// Okno główne (tworzone ponownie po zniszczeniu WebView).
pub fn ensure_main(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(MAIN) {
        return Ok(window);
    }
    builder(app, MAIN, "index.html", data_dir(app))
        .title("Alfa")
        .inner_size(1200.0, 800.0)
        .min_inner_size(400.0, 500.0)
        .center()
        .resizable(true)
        .decorations(true)
        .visible(true)
        .build()
}

fn ensure_quick(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(QUICK) {
        return Ok(window);
    }
    builder(app, QUICK, "quick.html", data_dir(app))
        .title("Alfa — Szybkie pytanie")
        .inner_size(640.0, 420.0)
        .center()
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .build()
}

fn ensure_pill(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(PILL) {
        return Ok(window);
    }
    builder(app, PILL, "pill.html", data_dir(app))
        .title("Alfa — głos")
        .inner_size(220.0, 48.0)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .build()
}

/// Tworzy okna przy starcie: główne widoczne, Szybkie pytanie i pigułka ukryte.
pub fn create_all(app: &AppHandle) -> tauri::Result<()> {
    ensure_main(app)?;
    ensure_quick(app)?;
    ensure_pill(app)?;
    Ok(())
}

/// Pokazuje okno główne (z zasobnika, Szybkiego pytania, protokołu, drugiej instancji).
pub fn show_main(app: &AppHandle) -> tauri::Result<()> {
    app.state::<WindowState>().cancel_timer();
    let window = ensure_main(app)?;
    window.unminimize()?;
    window.show()?;
    window.set_focus()
}

/// Przełącza okno Szybkiego pytania (`Ctrl+Alt+Space`).
pub fn toggle_quick(app: &AppHandle) -> tauri::Result<()> {
    let window = ensure_quick(app)?;
    if window.is_visible()? {
        return window.hide();
    }
    window.center()?;
    window.show()?;
    window.set_focus()
}

/// Chowa okno Szybkiego pytania.
pub fn hide_quick(app: &AppHandle) -> tauri::Result<()> {
    match app.get_webview_window(QUICK) {
        Some(window) => window.hide(),
        None => Ok(()),
    }
}

/// Czy okno główne jest widoczne (powiadomienia natywne tylko, gdy nie).
pub fn main_visible(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

fn setting_minutes(value: Option<SettingValue>) -> u64 {
    match value {
        Some(SettingValue::Number(n)) => n.as_u64().unwrap_or(10),
        _ => 10,
    }
    .clamp(1, 24 * 60)
}

/// Zamknięcie okna: główne → ukrycie (do zasobnika) i zniszczenie WebView po N minutach;
/// Szybkie pytanie i pigułka → ukrycie.
pub fn on_close_requested(window: &Window, api: &CloseRequestApi) {
    let app = window.app_handle().clone();
    let label = window.label().to_owned();
    if label != MAIN {
        api.prevent_close();
        let _ = window.hide();
        return;
    }
    let core = app.state::<AppCore>().inner().clone();
    let close_to_tray = tauri::async_runtime::block_on(core.setting("general.close_to_tray"));
    if matches!(close_to_tray, Some(SettingValue::Bool(false))) {
        app.exit(0);
        return;
    }
    api.prevent_close();
    let _ = window.hide();
    let state = app.state::<WindowState>();
    state.cancel_timer();
    let later = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let minutes = setting_minutes(core.setting("general.destroy_webview_after").await);
        tokio::time::sleep(Duration::from_secs(minutes * 60)).await;
        if let Some(main) = later.get_webview_window(MAIN)
            && !main.is_visible().unwrap_or(true)
        {
            let _ = main.destroy();
        }
    });
    if let Ok(mut timer) = state.destroy_timer.lock() {
        *timer = Some(task);
    }
}
