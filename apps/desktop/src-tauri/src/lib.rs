//! Powłoka Alfy (Tauri 2 + WebView2): składa `AppCore` (crates/app-core), rejestruje komendy IPC
//! z COMMANDS.md, emituje paczki zdarzeń na `alfa://events`, prowadzi okna `main`/`quick`/`pill`,
//! zasobnik, skróty globalne, powiadomienia, jedną instancję i protokół `alfa://`.
//! Logika aplikacji jest w `app-core`; tu tylko kleje systemowe.

mod boot;
mod cdp;
mod commands;
mod kernel;
mod logs;
mod pump;
mod shell;
mod shortcuts;
mod tray;
mod windows;

use app_core::{AppCore, AppOptions, AppPaths};
use tauri::{Manager, WindowEvent};

/// Uruchamia aplikację. Błąd startu jest fatalny — nie ma sensownego stanu bez rdzenia i okna.
pub fn run() {
    // Dziennik przed wszystkim innym — błędy startu Brokera i rdzenia też trafiają do pliku.
    let log_handle = logs::start();
    if let Some(why) = cdp::environment_violation(|name| std::env::var(name).ok()) {
        tracing::error!(powod = %why, "start wstrzymany: środowisko WebView2 (F3-13)");
        eprintln!("alfa-desktop: {why}");
        boot::mark_failed(&format!("środowisko WebView2: {why}"));
        std::process::exit(1);
    }
    let mut builder = tauri::Builder::default();
    // Single-instance musi być pierwszą wtyczką: druga instancja przekazuje argumenty/URI.
    // Wyjątek tylko w buildzie testowym: instancja testowa na własnym profilu działa obok
    // uruchomionej Alfy właściciela (`cdp::test_instance`, docs/plan/24 §2.7).
    if !cdp::test_instance() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            shell::handle_args(app, argv);
        }));
    }
    let result = builder
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
        .setup(move |app| {
            tracing::info!(wersja = %app.package_info().version, "start powłoki Alfy");
            app.manage(boot::BootLog(log_handle.clone()));
            // Po wtyczce jednej instancji: druga instancja (zaraz zakończona) nie zabierze znacznika.
            boot::report_previous_failure();
            let mut paths = AppPaths::from_env().map_err(|e| e.message)?;
            // Instancja testowa: katalogi robocze sesji zostają w profilu testu, nie w katalogu
            // `Alfa` właściciela (`%USERPROFILE%` jest wspólny dla całego konta Windows).
            if cdp::test_instance() {
                paths.user_root = paths.local.join("uzytkownik-testowy");
            }
            // Silniki osierocone przez awarię poprzedniego uruchomienia trzymałyby pamięć karty
            // graficznej — kończymy je, zanim rdzeń zacznie uruchamiać nowe.
            let _ = std::fs::create_dir_all(paths.state());
            let orphans = lib_sidecar_guard::init(paths.state().join("sidecars.pids"));
            if orphans > 0 {
                tracing::warn!(
                    orphans,
                    "zakończono procesy poboczne poprzedniego uruchomienia"
                );
            }
            let handle = app.handle().clone();
            // Procesy Jądra przed rdzeniem: Broker poza procesem (usługa / tryb przenośny),
            // watchdog z kill-switchem; release bez izolowanego Brokera — bezpieczny stan.
            let (broker, kernel_processes) = kernel::start();
            let options = AppOptions {
                app_version: app.package_info().version.to_string(),
                shell: Some(std::sync::Arc::new(shell::TauriShell::new(handle.clone()))),
                kernel: broker,
                // Instancja testowa obok Alfy właściciela: własne wpisy `Alfa/test/…` w magazynie
                // poświadczeń — nie odczyta ani nie nadpisze kluczy właściciela.
                secrets: if cdp::test_instance() {
                    Some(app_core::scoped_system_store("test").map_err(|e| e.to_string())?)
                } else {
                    None
                },
                ..AppOptions::default()
            };
            let core = tauri::async_runtime::block_on(AppCore::build(paths.clone(), options))
                .map_err(|e| e.message)?;
            if let Some(log_handle) = &log_handle {
                logs::apply_settings(log_handle, &core);
            }
            app.manage(core.clone());
            let hide = tauri::async_runtime::block_on(core.setting(windows::HIDE_FROM_CAPTURE));
            app.manage(windows::WindowState::new(
                paths.webview_data(),
                windows::hide_from_capture(hide.as_ref()),
            ));
            windows::create_all(&handle)?;
            tray::build(&handle)?;
            shortcuts::register(&handle, &core);
            kernel::watch(&handle, &core, &kernel_processes);
            app.manage(kernel_processes);
            pump::spawn(handle.clone(), core);
            shell::handle_args(&handle, std::env::args().collect());
            boot::finish_later(log_handle.clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .map(|app| {
            app.run(|_, event| {
                // Wyjście przez `exit` nie niszczy obiektów procesów (`kill_on_drop` nie zadziała):
                // silniki poboczne kończymy jawnie, inaczej zostają i trzymają pamięć karty.
                if let tauri::RunEvent::Exit = event {
                    let closed = lib_sidecar_guard::kill_all();
                    tracing::info!(closed, "zamknięto procesy poboczne przy wyjściu");
                }
            });
        });
    if let Err(error) = result {
        tracing::error!(error = %error, "błąd uruchomienia Tauri");
        eprintln!("alfa-desktop: błąd uruchomienia Tauri: {error}");
        boot::mark_failed(&error.to_string());
        std::process::exit(1);
    }
}
