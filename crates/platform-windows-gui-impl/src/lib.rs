//! `platform-windows` v2 (F6; docs/modules/platform-windows/SPEC.md, PLAN §7): okna v2,
//! UI Automation, wejście syntetyczne i zrzuty dla Windows — implementacja portów
//! `DesktopPort`, `UiaPort`, `InputPort`, `ScreenCapturePort` z `platform-contract`.
//!
//! - **UIA**: dedykowany wątek COM MTA z limitem czasu każdego wywołania; zawieszony wątek jest
//!   porzucany i zastępowany (limit wiszących), `IUIAutomation2` z limitami połączenia i transakcji;
//!   jedno wywołanie międzyprocesowe na węzeł (pamięć podręczna UIA).
//! - **Wejście**: `SendInput` paczkami atomowymi przez wspólny `execute_input` z kontraktu
//!   (cel i strażnik tuż przed każdą paczką), hook `WH_KEYBOARD_LL`/`WH_MOUSE_LL` wykrywa fizyczne
//!   wejście użytkownika (niewstrzyknięte) → przerwanie; tempo z `InputPacing`.
//! - **Zrzuty**: BitBlt (`CAPTUREBLT`) / `PrintWindow(PW_RENDERFULLCONTENT)`, maskowanie okien
//!   chronionych, aplikacji z deny-listy i pól haseł (UIA `IsPassword`), skalowanie, PNG (`flate2`).
//! - **Strażnik**: okna procesów Alfy/Brokera/helpera i procesów nieznanych nigdy nie są celem.
//!
//! Wydzielone z `platform-windows-impl` (limit rozmiaru crate'a). Poza Windows porty zwracają
//! `Unsupported` (logikę testuje się na `platform-fake`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
mod backend;
#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod desktop;
#[cfg(windows)]
mod hook;
#[cfg(windows)]
mod input;
#[cfg(not(windows))]
mod portable;
#[cfg(windows)]
mod uia;
#[cfg(windows)]
mod win;
mod zlib;

#[cfg(not(windows))]
use portable as backend;

use platform_contract::{
    CaptureRequest, DesktopPort, DesktopWindow, ElementRef, GuiError, InputControl, InputPacing,
    InputPlan, InputPort, InputReport, MonitorInfo, ScreenCapturePort, ScreenRect, Screenshot,
    TargetGuard, TreeOptions, UIA_CALL_TIMEOUT_MS, UIA_TREE_TIMEOUT_MS, UiaAction, UiaNode,
    UiaPort, UiaQuery, UiaText, UiaTree, WindowId, WindowState,
};

/// Konfiguracja (`[platform.gui]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiConfig {
    /// Strażnik celów (procesy Alfy/Brokera, PID-y usług, katalog instalacji).
    pub guard: TargetGuard,
    /// Tempo i limity wejścia.
    pub pacing: InputPacing,
    /// Limit pojedynczego wywołania UIA (ms).
    pub uia_call_timeout_ms: u64,
    /// Limit odczytu drzewa / wyszukiwania (ms).
    pub uia_tree_timeout_ms: u64,
    /// Ile porzuconych, wiszących wątków UIA naraz (potem odmowa).
    pub max_hung_uia_threads: usize,
    /// Budżet sprawdzania pól haseł przy zrzucie (ms; po nim okna maskowane w całości).
    pub capture_password_budget_ms: u64,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            guard: TargetGuard::baseline(),
            pacing: InputPacing::default(),
            uia_call_timeout_ms: UIA_CALL_TIMEOUT_MS,
            uia_tree_timeout_ms: UIA_TREE_TIMEOUT_MS,
            max_hung_uia_threads: 4,
            capture_password_budget_ms: 4_000,
        }
    }
}

/// Porty GUI Windows.
#[derive(Debug)]
pub struct WinGui {
    config: GuiConfig,
    backend: backend::Backend,
}

impl WinGui {
    /// Porty z konfiguracji (wątki UIA i hooka startują leniwie).
    pub fn new(config: GuiConfig) -> Self {
        let backend = backend::Backend::new(&config);
        Self { config, backend }
    }

    /// Konfiguracja.
    pub fn config(&self) -> &GuiConfig {
        &self.config
    }
}

impl Default for WinGui {
    fn default() -> Self {
        Self::new(GuiConfig::default())
    }
}

impl DesktopPort for WinGui {
    fn guard(&self) -> &TargetGuard {
        &self.config.guard
    }
    fn windows(&self) -> Result<Vec<DesktopWindow>, GuiError> {
        self.backend.windows(&self.config)
    }
    fn foreground(&self) -> Result<Option<DesktopWindow>, GuiError> {
        self.backend.foreground(&self.config)
    }
    fn window_at(&self, x: i32, y: i32) -> Result<Option<WindowId>, GuiError> {
        self.backend.window_at(x, y)
    }
    fn monitors(&self) -> Result<Vec<MonitorInfo>, GuiError> {
        self.backend.monitors()
    }
    fn focus(&self, id: WindowId) -> Result<(), GuiError> {
        self.backend.focus(&self.config, id)
    }
    fn set_bounds(&self, id: WindowId, rect: ScreenRect) -> Result<(), GuiError> {
        self.backend.set_bounds(&self.config, id, rect)
    }
    fn set_state(&self, id: WindowId, state: WindowState) -> Result<(), GuiError> {
        self.backend.set_state(&self.config, id, state)
    }
}

impl UiaPort for WinGui {
    fn tree(&self, window: WindowId, options: &TreeOptions) -> Result<UiaTree, GuiError> {
        self.backend.uia_tree(&self.config, window, options)
    }
    fn find(&self, window: WindowId, query: &UiaQuery) -> Result<Vec<UiaNode>, GuiError> {
        self.backend.uia_find(&self.config, window, query)
    }
    fn element(&self, element: &ElementRef) -> Result<UiaNode, GuiError> {
        self.backend.uia_element(&self.config, element)
    }
    fn read_text(&self, element: &ElementRef, max_chars: usize) -> Result<UiaText, GuiError> {
        self.backend.uia_read_text(&self.config, element, max_chars)
    }
    fn act(&self, element: &ElementRef, action: &UiaAction) -> Result<UiaNode, GuiError> {
        self.backend.uia_act(&self.config, element, action)
    }
    fn password_rects(&self, window: WindowId) -> Result<Vec<ScreenRect>, GuiError> {
        self.backend
            .uia_password_rects(&self.config, window, self.config.uia_call_timeout_ms)
    }
}

impl InputPort for WinGui {
    fn send(&self, plan: &InputPlan, control: &InputControl) -> Result<InputReport, GuiError> {
        self.backend.send(&self.config, plan, control)
    }
}

impl ScreenCapturePort for WinGui {
    fn capture(&self, request: &CaptureRequest) -> Result<Screenshot, GuiError> {
        self.backend.capture(&self.config, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_ports<T: DesktopPort + UiaPort + InputPort + ScreenCapturePort>(_: &T) {}

    #[test]
    fn ports_and_defaults() {
        let gui = WinGui::default();
        assert_ports(&gui);
        assert_eq!(gui.config().max_hung_uia_threads, 4);
        assert!(gui.guard().is_protected(1, "alfa-broker-ui.exe"));
        if !cfg!(windows) {
            assert!(gui.windows().is_err());
            assert!(gui.tree(WindowId(1), &TreeOptions::default()).is_err());
            let plan = InputPlan {
                window: WindowId(1),
                steps: vec![],
            };
            assert!(gui.send(&plan, &InputControl::new()).is_err());
        }
    }
}
