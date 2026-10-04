//! Kompilacyjny test kontraktu powłoki: każda komenda z `with_commands!` (z której powłoka Tauri
//! generuje handlery) istnieje w `AppCore` z tymi typami, a jej przyszłość jest `Send + 'static`
//! (wymóg komend asynchronicznych Tauri). Błąd = błąd kompilacji tego testu na Linuksie,
//! zanim powłoka trafi do kompilatora na Windows.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::future::Future;

use app_core::{AppCore, AppError};

macro_rules! check_signatures {
    ($( $name:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty ; )*) => {
        $(
            fn $name(core: AppCore, $( $arg: $ty ),*)
                -> impl Future<Output = Result<$ret, AppError>> + Send + 'static
            {
                async move { core.$name($( $arg ),*).await }
            }
        )*

        const CHECKED: &[&str] = &[$( stringify!($name) ),*];
    };
}

app_core::with_commands!(check_signatures);

/// Komenda ze strumieniem w kanale (handler ręczny w powłoce): odbiorca zamiast `Channel`.
/// Jawny typ zwracany jest celem testu (`Send + 'static`), stąd bez `async fn`.
#[allow(clippy::manual_async_fn)]
fn terminal_open(
    core: AppCore,
    sink: std::sync::Arc<dyn app_core::FrameSink>,
) -> impl Future<Output = Result<app_core::dto::TerminalSession, AppError>> + Send + 'static {
    async move {
        core.terminal_open(app_core::dto::TerminalProfileId::Shell, 80, 24, None, sink)
            .await
    }
}

#[test]
fn every_command_has_a_send_future_with_matching_types() {
    let all: Vec<&str> = CHECKED
        .iter()
        .chain(app_core::CHANNEL_COMMANDS)
        .copied()
        .collect();
    assert_eq!(all, app_core::COMMANDS);
    assert_eq!(CHECKED.len(), 183);
    assert_eq!(app_core::CHANNEL_COMMANDS, ["terminal_open"]);
    let _ = terminal_open;
}
