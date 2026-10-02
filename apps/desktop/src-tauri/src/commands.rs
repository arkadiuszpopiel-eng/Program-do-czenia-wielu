//! Komendy IPC (COMMANDS.md) generowane z listy `app_core::with_commands!` (jedno źródło,
//! sprawdzane testem `crates/app-core/tests/ipc_signatures.rs`): każda deleguje do
//! `AppCore::<nazwa>`, a błąd (`AppError`) wraca jako tekst w języku interfejsu (odrzucenie
//! `invoke` w UI). Argumenty z JS w camelCase Tauri mapuje na parametry snake_case.
//! Uprawnienia: `allow-<komenda>` per okno (`capabilities/*.json`). Komendy ze strumieniem
//! (`app_core::CHANNEL_COMMANDS`: `terminal_open`) mają handler ręczny z `tauri::ipc::Channel`.

use std::sync::Arc;

use app_core::AppCore;
use app_core::dto::{TerminalFrame, TerminalProfileId, TerminalSession};
use tauri::ipc::Channel;

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

        /// Obsługa wszystkich komend aplikacji (lista `with_commands!` + komendy ze strumieniem).
        pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
            tauri::generate_handler![$( $name, )* terminal_open]
        }
    };
}

app_core::with_commands!(tauri_commands);

/// `terminal_open` — wywołuje ją wyłącznie panel terminala z gestu użytkownika (kliknięcie,
/// klawisz). Wyjście VT idzie **tylko** kanałem `channel` do tego panelu — nigdy przez
/// `alfa://events`, magistralę ani logi (rdzeń nie widzi kanału, tylko odbiorcę ramek).
#[tauri::command]
async fn terminal_open(
    core: tauri::State<'_, AppCore>,
    profile: TerminalProfileId,
    cols: u16,
    rows: u16,
    cwd: Option<String>,
    channel: Channel<TerminalFrame>,
) -> Result<TerminalSession, String> {
    let core = core.inner().clone();
    let sink: Arc<dyn app_core::FrameSink> = Arc::new(move |frame: TerminalFrame| {
        // Zamknięty panel (kanał bez odbiorcy) — ramka przepada, nic nie jest buforowane.
        let _ = channel.send(frame);
    });
    match core.terminal_open(profile, cols, rows, cwd, sink).await {
        Ok(value) => Ok(value),
        Err(error) => Err(core.error_text(&error).await),
    }
}
