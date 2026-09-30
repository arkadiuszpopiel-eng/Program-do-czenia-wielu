//! Powłoka Alfy: minimalny `run()` uruchamiający okno główne z UI Svelte 5 (`../ui`).
//! F0: bez komend IPC; komendy i zdarzenia rdzenia dojdą wraz z kontraktami `core-*`.

/// Uruchamia aplikację Tauri. Błąd startu jest fatalny — nie ma sensownego stanu bez okna.
pub fn run() {
    let result = tauri::Builder::default().run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("alfa-desktop: błąd uruchomienia Tauri: {error}");
        std::process::exit(1);
    }
}
