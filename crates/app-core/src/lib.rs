//! Korzeń kompozycji Alfy (`app-*`, crates/README.md): składa moduły `-impl` w kolejności
//! z `core-registry` i wystawia każdą komendę z `apps/desktop/ui/src/lib/api/COMMANDS.md` jako
//! metodę `AppCore::<przestrzeń>_<nazwa>` oraz strumień zdarzeń `alfa://events` (paczki co klatkę).
//! Powłoka Tauri (`apps/desktop/src-tauri`) jest cienka: deleguje tu każdą komendę.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod chat;
mod commands;
mod commands_list;
mod compose;
mod core;
mod infra;
mod lifecycle;
mod options;
mod parts;
mod route;
mod settings;
mod store;
mod store_agents;
mod voice_chat;

// Kontrakt IPC (DTO, błędy, zdarzenia, porty) — crate `app-api`, pod dawnymi ścieżkami.
use app_api::error;
pub use app_api::{dto, events, ids, notify, ports, protocol};

pub use crate::core::AppCore;
pub use app_agents::eval;
pub use app_api::{AppError, DEFAULT_FRAME, ErrorCode, EventBatch};
pub use app_modules::NO_TTS;
pub use app_voice::{Pacer, VoiceEngine, VoiceEngineFactory};
pub use commands::app::SYSTEM_SETTINGS_ALLOWED;
pub use commands::settings::{KILL_SWITCH_CHORD, validate_chord};
pub use commands_list::COMMANDS;
pub use infra::secrets::MemorySecretStore;
pub use options::{AppOptions, AppPaths};
pub use providers_contract::CancellationToken;
pub use route::{LOCAL_PROVIDER, NO_BRAIN, NO_LOCAL, RouterBrain};
pub use router_contract::RouteKind;
