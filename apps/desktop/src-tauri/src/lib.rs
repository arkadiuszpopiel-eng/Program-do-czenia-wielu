//! Powłoka Alfy (Tauri 2 + WebView2): składa `AppCore` (crates/app-core), rejestruje komendy IPC
//! z COMMANDS.md, emituje paczki zdarzeń na `alfa://events`, prowadzi okna `main`/`quick`/`pill`,
//! zasobnik, skróty globalne, powiadomienia, jedną instancję i protokół `alfa://`.
//! Logika aplikacji jest w `app-core`; tu tylko kleje systemowe.

mod commands;
mod kernel;
mod pump;
mod shell;
mod shortcuts;
mod tray;
mod windows;

use app_core::{AppCore, AppOptions, AppPaths};
use tauri::{Manager, WindowEvent};

/// Uruchamia aplikację. Błąd startu jest fatalny — nie ma sensownego stanu bez rdzenia i okna.
pub fn run() {
    let result = tauri::Builder::default()
        // Single-instance musi być pierwszą wtyczką: druga instancja przekazuje argumenty/URI.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            shell::handle_args(app, argv);
        }))
        .plugin(shortcuts::plugin())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(commands::handler())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                windows::on_close_requested(window, api);
            }
            if let WindowEvent::Focused(false) = event
                && window.label() == windows::QUICK
            {
                let _ = window.hide();
            }
            // Pliki upuszczone na okno główne: ścieżki z systemu trafiają do rdzenia, UI pobiera je
            // komendą `attachments_add_dropped` (WebView nie podaje ścieżek — `app-files`).
            if let WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event
                && window.label() == windows::MAIN
                && let Some(core) = window.try_state::<AppCore>()
            {
                core.attachments_dropped(paths.clone());
            }
        })
        .setup(|app| {
            let paths = AppPaths::from_env().map_err(|e| e.message)?;
            let handle = app.handle().clone();
            // Procesy Jądra przed rdzeniem: Broker poza procesem (usługa / tryb przenośny),
            // watchdog z kill-switchem; release bez izolowanego Brokera — bezpieczny stan.
            let (broker, kernel_processes) = kernel::start();
            let options = AppOptions {
                app_version: app.package_info().version.to_string(),
                shell: Some(std::sync::Arc::new(shell::TauriShell::new(handle.clone()))),
                kernel: broker,
                ..AppOptions::default()
            };
            let core = tauri::async_runtime::block_on(AppCore::build(paths.clone(), options))
                .map_err(|e| e.message)?;
            app.manage(core.clone());
            app.manage(windows::WindowState::new(paths.webview_data()));
            windows::create_all(&handle)?;
            tray::build(&handle)?;
            shortcuts::register(&handle, &core);
            kernel::watch(&handle, &core, &kernel_processes);
            app.manage(kernel_processes);
            pump::spawn(handle.clone(), core);
            shell::handle_args(&handle, std::env::args().collect());
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("alfa-desktop: błąd uruchomienia Tauri: {error}");
        std::process::exit(1);
    }
}
