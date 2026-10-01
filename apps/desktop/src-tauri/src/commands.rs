//! Komendy IPC (COMMANDS.md) generowane z listy `app_core::with_commands!` (jedno źródło,
//! sprawdzane testem `crates/app-core/tests/ipc_signatures.rs`): każda deleguje do
//! `AppCore::<nazwa>`, a błąd (`AppError`) wraca jako tekst w języku interfejsu (odrzucenie
//! `invoke` w UI). Argumenty z JS w camelCase Tauri mapuje na parametry snake_case.
//! Uprawnienia: `allow-<komenda>` per okno (`capabilities/*.json`).

use app_core::AppCore;

macro_rules! tauri_commands {
    ($( $name:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty ; )*) => {
        $(
            #[tauri::command]
            async fn $name(
                core: tauri::State<'_, AppCore>,
                $( $arg: $ty ),*
            ) -> Result<$ret, String> {
                let core = core.inner().clone();
                match core.$name($( $arg ),*).await {
                    Ok(value) => Ok(value),
                    Err(error) => Err(core.error_text(&error).await),
                }
            }
        )*

        /// Obsługa wszystkich komend aplikacji.
        pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
            tauri::generate_handler![$( $name ),*]
        }
    };
}

app_core::with_commands!(tauri_commands);
