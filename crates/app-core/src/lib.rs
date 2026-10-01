//! Korzeń kompozycji Alfy (`app-*`, crates/README.md): składa moduły `-impl` w kolejności
//! z `core-registry` i wystawia każdą komendę z `apps/desktop/ui/src/lib/api/COMMANDS.md` jako
//! metodę `AppCore::<przestrzeń>_<nazwa>` oraz strumień zdarzeń `alfa://events` (paczki co klatkę).
//! Powłoka Tauri (`apps/desktop/src-tauri`) jest cienka: deleguje tu każdą komendę.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod brain;
mod chat;
mod commands;
mod commands_list;
mod compose;
mod core;
pub mod dto;
mod error;
pub mod events;
mod ids;
mod infra;
mod lifecycle;
pub mod notify;
mod options;
mod parts;
pub mod ports;
pub mod protocol;
mod settings;
mod store;

pub use crate::core::AppCore;
pub use brain::{NO_BRAIN, NO_LOCAL};
pub use commands::app::SYSTEM_SETTINGS_ALLOWED;
pub use commands::settings::{KILL_SWITCH_CHORD, validate_chord};
pub use commands_list::COMMANDS;
pub use error::{AppError, ErrorCode};
pub use events::{DEFAULT_FRAME, EventBatch};
pub use infra::secrets::MemorySecretStore;
pub use options::{AppOptions, AppPaths};
pub use providers_contract::CancellationToken;
