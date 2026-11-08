//! Computer use w aplikacji (kategoria `app-*`, wydzielona z `app-core` — limit rozmiaru crate'a):
//! - [`GuiPorts`] — porty GUI Windows (`WinGui` z `platform-windows-gui-impl`, budowany leniwie)
//!   ze strażnikiem celów: lista bazowa + drzewo procesów Alfy (WebView2, Broker/watchdog, terminal)
//!   + katalogi instalacji (`%LOCALAPPDATA%\Alfa`, katalog programu); w testach — `platform-fake`;
//! - [`gui_tools`] — zestawy `tools-window` / `tools-uia` / `tools-input` / `tools-screen` nad
//!   Brokerem (każda akcja `gui.control(...)`), opakowane przez [`GuiMonitor`]; rejestr narzędzi
//!   agentek przydziela je rolom z grupą `gui.control` (`ToolManifest::allowed_for`);
//! - [`GuiMonitor`] / [`GuiApp`] — panel „Ekran": co agentka robi i widzi (ostatni zamaskowany
//!   zrzut tylko w pamięci), wskaźnik „agentka steruje", przejęcie sterowania, „zawsze zezwalaj
//!   na podgląd pulpitu" przez Brokera.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod app;
mod describe;
mod monitor;
mod ports;

use std::sync::Arc;

use core_bus_contract::EventBus;
use safety_broker_contract::Broker;
use tools_common_contract::{Tool, Toolset};
use tools_input_impl::{InputTools, InputToolsDeps};
use tools_screen_impl::{ScreenTools, ScreenToolsDeps};
use tools_uia_impl::{UiaTools, UiaToolsDeps};
use tools_window_impl::{WindowTools, WindowToolsDeps};

pub use app::GuiApp;
pub use monitor::{CONTROL_LINGER, GuiMonitor, MAX_ACTIONS, WatchedTool};
pub use ports::{GuiPorts, LazyWinGui, alfa_guard, descendants};

/// Manifesty modułów składanych przez ten crate (identyfikator → `module.toml`).
pub const MODULES: &[(&str, &str)] = &[
    ("tools-window", tools_window_impl::MODULE_TOML),
    ("tools-uia", tools_uia_impl::MODULE_TOML),
    ("tools-input", tools_input_impl::MODULE_TOML),
    ("tools-screen", tools_screen_impl::MODULE_TOML),
];

/// Narzędzia GUI agentek nad portami i Brokerem, pod nadzorem panelu „Ekran".
pub fn gui_tools(
    ports: &GuiPorts,
    broker: Arc<dyn Broker>,
    bus: Option<Arc<dyn EventBus>>,
    monitor: &Arc<GuiMonitor>,
) -> Vec<Arc<dyn Tool>> {
    let mut tools = WindowTools::new(WindowToolsDeps {
        desktop: ports.desktop.clone(),
        broker: broker.clone(),
        bus: bus.clone(),
    })
    .tools();
    tools.extend(
        UiaTools::new(UiaToolsDeps {
            desktop: ports.desktop.clone(),
            uia: ports.uia.clone(),
            broker: broker.clone(),
            config: tools_uia_contract::UiaToolsConfig::default(),
            bus: bus.clone(),
        })
        .tools(),
    );
    tools.extend(
        InputTools::new(InputToolsDeps {
            desktop: ports.desktop.clone(),
            uia: ports.uia.clone(),
            input: ports.input.clone(),
            broker: broker.clone(),
            config: tools_input_contract::InputToolsConfig::default(),
            bus: bus.clone(),
        })
        .tools(),
    );
    tools.extend(
        ScreenTools::new(ScreenToolsDeps {
            desktop: ports.desktop.clone(),
            capture: ports.capture.clone(),
            broker,
            config: tools_screen_contract::ScreenToolsConfig::default(),
            bus,
        })
        .tools(),
    );
    monitor.wrap(tools)
}
