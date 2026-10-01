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

#[test]
fn every_command_has_a_send_future_with_matching_types() {
    assert_eq!(CHECKED, app_core::COMMANDS);
    assert_eq!(CHECKED.len(), 76);
}
