//! Kontrakt IPC Alfy (kategoria `app-*`): DTO komend i zdarzeń 1:1 z
//! `apps/desktop/ui/src/lib/api/types*.ts`, błąd komend (`AppError`), identyfikatory DTO niosące
//! sesję, strumień paczek zdarzeń co klatkę (`EventHub`), porty powłoki i modułów podmieniane
//! w `AppOptions`, protokół `alfa://` i powiadomienia natywne. Wydzielony z `app-core` (limit
//! rozmiaru crate'a); `app-core` reeksportuje wszystkie moduły pod tymi samymi ścieżkami.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod commands;
pub mod details;
pub mod dto;
pub mod error;
pub mod event_log;
pub mod events;
pub mod ids;
pub mod local_only;
pub mod notify;
pub mod paths;
pub mod ports;
pub mod protocol;
pub mod schema;

pub use commands::{CHANNEL_COMMANDS, COMMANDS};
pub use error::{AppError, ErrorCode};
pub use events::{DEFAULT_FRAME, EventBatch, EventHub};
pub use paths::AppPaths;
