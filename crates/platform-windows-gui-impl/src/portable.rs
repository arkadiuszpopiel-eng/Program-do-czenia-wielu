//! Poza Windows: porty GUI niedostępne (`Unsupported`) — logikę testuje się na `platform-fake`.

use platform_contract::{
    CaptureRequest, DesktopWindow, ElementRef, GuiError, InputControl, InputPlan, InputReport,
    MonitorInfo, PlatformError, ScreenRect, Screenshot, TreeOptions, UiaAction, UiaNode, UiaQuery,
    UiaText, UiaTree, WindowId, WindowState,
};

use crate::GuiConfig;

fn unsupported<T>(what: &str) -> Result<T, GuiError> {
    Err(GuiError::Platform(PlatformError::Unsupported(format!(
        "{what}: tylko Windows"
    ))))
}

/// Pusty backend.
#[derive(Debug)]
pub(crate) struct Backend;

impl Backend {
    pub(crate) fn new(_: &GuiConfig) -> Self {
        Self
    }
    pub(crate) fn windows(&self, _: &GuiConfig) -> Result<Vec<DesktopWindow>, GuiError> {
        unsupported("lista okien")
    }
    pub(crate) fn foreground(&self, _: &GuiConfig) -> Result<Option<DesktopWindow>, GuiError> {
        unsupported("okno pierwszego planu")
    }
    pub(crate) fn window_at(&self, _: i32, _: i32) -> Result<Option<WindowId>, GuiError> {
        unsupported("okno pod punktem")
    }
    pub(crate) fn monitors(&self) -> Result<Vec<MonitorInfo>, GuiError> {
        unsupported("monitory")
    }
    pub(crate) fn focus(&self, _: &GuiConfig, _: WindowId) -> Result<(), GuiError> {
        unsupported("fokus")
    }
    pub(crate) fn set_bounds(
        &self,
        _: &GuiConfig,
        _: WindowId,
        _: ScreenRect,
    ) -> Result<(), GuiError> {
        unsupported("położenie okna")
    }
    pub(crate) fn set_state(
        &self,
        _: &GuiConfig,
        _: WindowId,
        _: WindowState,
    ) -> Result<(), GuiError> {
        unsupported("stan okna")
    }
    pub(crate) fn uia_tree(
        &self,
        _: &GuiConfig,
        _: WindowId,
        _: &TreeOptions,
    ) -> Result<UiaTree, GuiError> {
        unsupported("UIA")
    }
    pub(crate) fn uia_find(
        &self,
        _: &GuiConfig,
        _: WindowId,
        _: &UiaQuery,
    ) -> Result<Vec<UiaNode>, GuiError> {
        unsupported("UIA")
    }
    pub(crate) fn uia_element(&self, _: &GuiConfig, _: &ElementRef) -> Result<UiaNode, GuiError> {
        unsupported("UIA")
    }
    pub(crate) fn uia_read_text(
        &self,
        _: &GuiConfig,
        _: &ElementRef,
        _: usize,
    ) -> Result<UiaText, GuiError> {
        unsupported("UIA")
    }
    pub(crate) fn uia_act(
        &self,
        _: &GuiConfig,
        _: &ElementRef,
        _: &UiaAction,
    ) -> Result<UiaNode, GuiError> {
        unsupported("UIA")
    }
    pub(crate) fn uia_password_rects(
        &self,
        _: &GuiConfig,
        _: WindowId,
        _: u64,
    ) -> Result<Vec<ScreenRect>, GuiError> {
        unsupported("UIA")
    }
    pub(crate) fn send(
        &self,
        _: &GuiConfig,
        _: &InputPlan,
        _: &InputControl,
    ) -> Result<InputReport, GuiError> {
        unsupported("wejście syntetyczne")
    }
    pub(crate) fn capture(
        &self,
        _: &GuiConfig,
        _: &CaptureRequest,
    ) -> Result<Screenshot, GuiError> {
        unsupported("zrzut ekranu")
    }
}
