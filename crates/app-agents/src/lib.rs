//! Agentki z narzędziami w aplikacji (kategoria `app-*`, wydzielona z `app-core` — limit
//! rozmiaru crate'a):
//! - [`AgentTools`] — zestawy `tools-fs` / `tools-shell` / `tools-clipboard`, F6 `tools-office`
//!   i `tools-browser` ([`AppsDeps`]: Word/Excel przez COM, przeglądarka Alfy z profilem
//!   i kwarantanną w `%LOCALAPPDATA%\Alfa\browser`) oraz narzędzia wtyczek Wasm (`app-plugins`)
//!   nad jednym Brokerem (zwykle [`TicketLog`]) i dziennikiem cofania, filtr narzędzi rolami obsady,
//!   kill-switch zamykający przeglądarki; F6 `tools-system` i `tools-net` ([`SysNetDeps`]: procesy,
//!   usługi, Dziennik zdarzeń, zmienne, stan systemu, `net_fetch`/`net_download` z kwarantanną);
//!   F6 `tools-vision` i `tools-media` ([`MediaPorts`]: OCR, opis obrazu przez Router z tagiem
//!   prywatności sesji, nagłówki multimediów, ffmpeg jako sidecar, odtwarzanie w kolejce mówienia);
//! - [`TicketLog`] — przezroczysty dekorator Brokera zapamiętujący fakty próśb o zatwierdzenie
//!   (karta „czeka na zatwierdzenie");
//! - [`run_spec`] / [`AgentSettings`] — `RunSpec` z obsady i Ustawień → Agentki (budżety kroków,
//!   czasu i kosztu w PLN, limit czekania na zatwierdzenie, samoweryfikacja);
//! - [`RunHandle`] / [`RunFeed`] — przebieg `agent-runtime` na dostawcy z Routera i jego dziennik
//!   na żywo bez luk (steering, anulowanie);
//! - [`RunProjector`] — zdarzenia `agent.*` → Replay, linie kroków w wątku, karty „Cofnij",
//!   „czeka na zatwierdzenie", „uruchom w terminalu", kapsuła aktywności, Oś czasu;
//! - [`Launch`] / [`RunFamily`] / [`FamilyProjector`] — start v1 (`start_with` z obsadą: delegacja
//!   i Krytyczka; zasoby wyłączne, autonomia z Brokera; koperta umiejętności) i Replay podprzebiegów;
//! - [`eval`] — zestaw ewaluacyjny narzędzi F3 (`evals/F3/tools/`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod apps;
pub mod eval;
mod family;
mod launch;
mod map;
mod media;
mod project;
mod runner;
mod spec;
mod sysnet;
mod tickets;
mod toolset;

pub use apps::{AppsDeps, browser_spec};
pub use family::{ChildInfo, FamilyProjector, RunFamily};
pub use launch::{Launch, SkillCall};
pub use map::{FinalText, final_text, short};
pub use media::{MediaPorts, ffmpeg_path};
pub use project::{Projection, RunContext, RunProjector};
pub use runner::{RunFeed, RunHandle};
pub use spec::{AgentSettings, NO_WINDOW_APPROVAL_CAP_S, SpecInput, keys, run_spec};
pub use sysnet::{SysNetDeps, sysnet_guard};
pub use tickets::{TicketLog, TicketNote};
pub use tools_clipboard_contract::ClipboardUndoError;
pub use tools_media_impl::MediaTools;
pub use tools_shell_contract::ShellToolsConfig;
pub use tools_system_impl::EnvUndoError;
pub use toolset::{AgentTools, ToolsDeps};

/// Manifesty modułów składanych przez ten crate (identyfikator → `module.toml`) — rejestr
/// `core-registry` w `app-core` liczy z nich kolejność startu i pokazuje zdrowie.
pub const MODULES: &[(&str, &str)] = &[
    ("tools-fs", tools_fs_impl::MODULE_TOML),
    ("tools-shell", tools_shell_impl::MODULE_TOML),
    ("tools-clipboard", tools_clipboard_impl::MODULE_TOML),
    ("tools-office", tools_office_impl::MODULE_TOML),
    ("tools-browser", tools_browser_impl::MODULE_TOML),
    ("tools-system", tools_system_impl::MODULE_TOML),
    ("tools-net", tools_net_impl::MODULE_TOML),
    ("tools-vision", tools_vision_impl::MODULE_TOML),
    ("tools-media", tools_media_impl::MODULE_TOML),
    ("agent-runtime", agent_runtime_impl::MODULE_TOML),
];
