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
    assert_eq!(CHECKED.len(), 210);
    assert_eq!(app_core::CHANNEL_COMMANDS, ["terminal_open"]);
    let _ = terminal_open;
}

/// Każda komenda ma uprawnienie `allow-<komenda>` w co najmniej jednym oknie powłoki
/// (`apps/desktop/src-tauri/capabilities/*.json`). Brak wpisu = Tauri odrzuca wywołanie
/// („… not allowed”) tylko w prawdziwej aplikacji — atrapa UI tego nie widzi (pakiety 1–6,
/// 2026-10-07: `models_bundles not allowed` na laptopie, zielone testy UI).
#[test]
fn every_command_is_granted_to_some_window() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src-tauri/capabilities");
    let mut granted = std::collections::BTreeSet::new();
    for window in ["main", "quick", "pill"] {
        let text = std::fs::read_to_string(dir.join(format!("{window}.json"))).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        for p in json["permissions"].as_array().unwrap() {
            if let Some(name) = p.as_str().and_then(|p| p.strip_prefix("allow-")) {
                granted.insert(name.replace('-', "_"));
            }
        }
    }
    let missing: Vec<&str> = app_core::COMMANDS
        .iter()
        .copied()
        .filter(|c| !granted.contains(*c))
        .collect();
    assert!(
        missing.is_empty(),
        "komendy bez uprawnienia w capabilities/*.json: {missing:?}"
    );
}
