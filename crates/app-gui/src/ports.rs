//! Porty GUI dla narzędzi agentek: `WinGui` (`platform-windows-gui-impl`) budowany leniwie przy
//! pierwszej akcji — wtedy procesy WebView2 okien Alfy już istnieją, więc strażnik celów dostaje
//! PID-y całego drzewa procesów Alfy (WebView2, sidecary, terminal ConPTY, Broker i watchdog
//! uruchomione przez Alfę) oraz katalogi instalacji (`%LOCALAPPDATA%\Alfa`, katalog programu).
//! Obrazy Brokera, Broker-UI, watchdoga i helpera są chronione nazwą zawsze (lista bazowa).
//! Port i tak liczy drzewo procesów przy każdej akcji (potomek bieżącego procesu albo PID-u
//! z listy jest chroniony — np. proces WebView2 odtworzony po awarii; przegląd #2, P2-01).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use platform_contract::{
    CaptureRequest, DesktopPort, DesktopWindow, ElementRef, GuiError, InputControl, InputPlan,
    InputPort, InputReport, MonitorInfo, ScreenCapturePort, ScreenRect, Screenshot, TargetGuard,
    TreeOptions, UiaAction, UiaNode, UiaPort, UiaQuery, UiaText, UiaTree, WindowId, WindowState,
};
use platform_windows_gui_impl::{GuiConfig, WinGui};

/// Porty GUI (jedna implementacja dla czterech portów).
#[derive(Clone)]
pub struct GuiPorts {
    /// Okna.
    pub desktop: Arc<dyn DesktopPort>,
    /// UI Automation.
    pub uia: Arc<dyn UiaPort>,
    /// Wejście syntetyczne.
    pub input: Arc<dyn InputPort>,
    /// Zrzuty.
    pub capture: Arc<dyn ScreenCapturePort>,
    /// Czy platforma obsługuje GUI (poza Windows — `false`, narzędzia zwracają „nieobsługiwane").
    pub available: bool,
}

impl std::fmt::Debug for GuiPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiPorts")
            .field("available", &self.available)
            .finish_non_exhaustive()
    }
}

impl GuiPorts {
    /// Porty z jednej implementacji (np. `platform-fake::FakeDesktop` w testach).
    pub fn from_one<T>(gui: Arc<T>, available: bool) -> Self
    where
        T: DesktopPort + UiaPort + InputPort + ScreenCapturePort + 'static,
    {
        Self {
            desktop: gui.clone(),
            uia: gui.clone(),
            input: gui.clone(),
            capture: gui,
            available,
        }
    }

    /// Porty systemowe: `WinGui` ze strażnikiem okien Alfy (leniwie).
    pub fn system(local: &Path) -> Self {
        Self::from_one(Arc::new(LazyWinGui::new(local)), cfg!(windows))
    }
}

/// Katalog wykonywalnego Alfy (instalacja) — też chroniony.
fn program_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

/// Potomkowie procesu `root` (PID → rodzic) — drzewo procesów Alfy.
pub fn descendants(root: u32, parents: &BTreeMap<u32, u32>) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    let mut frontier = vec![root];
    while let Some(pid) = frontier.pop() {
        for (child, parent) in parents {
            if *parent == pid && *child != root && out.insert(*child) {
                frontier.push(*child);
            }
        }
    }
    out
}

/// Strażnik celów: lista bazowa + bieżący proces i całe jego drzewo + katalogi Alfy.
pub fn alfa_guard(local: &Path) -> TargetGuard {
    let processes = platform_windows_impl::WinProcesses::default().list();
    let parents: BTreeMap<u32, u32> = processes
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.pid, p.parent_pid))
        .collect();
    let me = std::process::id();
    let mut pids = vec![me];
    pids.extend(descendants(me, &parents));
    let mut dirs = vec![local.to_string_lossy().into_owned()];
    dirs.extend(program_dir().map(|d| d.to_string_lossy().into_owned()));
    TargetGuard::baseline()
        .with_pids(pids)
        .with_image_dirs(dirs)
}

/// `WinGui` budowany przy pierwszym użyciu (strażnik z PID-ami procesów istniejących wtedy).
pub struct LazyWinGui {
    local: PathBuf,
    gui: OnceLock<WinGui>,
}

impl LazyWinGui {
    /// Porty nad katalogiem danych Alfy (`%LOCALAPPDATA%\Alfa`).
    pub fn new(local: &Path) -> Self {
        Self {
            local: local.to_path_buf(),
            gui: OnceLock::new(),
        }
    }

    fn get(&self) -> &WinGui {
        self.gui.get_or_init(|| {
            let guard = alfa_guard(&self.local);
            tracing::info!(
                pids = guard.pids.len(),
                katalogi = guard.image_dirs.len(),
                "porty GUI: strażnik okien Alfy"
            );
            WinGui::new(GuiConfig {
                guard,
                ..GuiConfig::default()
            })
        })
    }
}

impl DesktopPort for LazyWinGui {
    fn guard(&self) -> &TargetGuard {
        self.get().guard()
    }
    fn windows(&self) -> Result<Vec<DesktopWindow>, GuiError> {
        self.get().windows()
    }
    fn foreground(&self) -> Result<Option<DesktopWindow>, GuiError> {
        self.get().foreground()
    }
    fn window_at(&self, x: i32, y: i32) -> Result<Option<WindowId>, GuiError> {
        self.get().window_at(x, y)
    }
    fn monitors(&self) -> Result<Vec<MonitorInfo>, GuiError> {
        self.get().monitors()
    }
    fn focus(&self, id: WindowId) -> Result<(), GuiError> {
        self.get().focus(id)
    }
    fn set_bounds(&self, id: WindowId, rect: ScreenRect) -> Result<(), GuiError> {
        self.get().set_bounds(id, rect)
    }
    fn set_state(&self, id: WindowId, state: WindowState) -> Result<(), GuiError> {
        self.get().set_state(id, state)
    }
}

impl UiaPort for LazyWinGui {
    fn tree(&self, window: WindowId, options: &TreeOptions) -> Result<UiaTree, GuiError> {
        self.get().tree(window, options)
    }
    fn find(&self, window: WindowId, query: &UiaQuery) -> Result<Vec<UiaNode>, GuiError> {
        self.get().find(window, query)
    }
    fn element(&self, element: &ElementRef) -> Result<UiaNode, GuiError> {
        self.get().element(element)
    }
    fn read_text(&self, element: &ElementRef, max_chars: usize) -> Result<UiaText, GuiError> {
        self.get().read_text(element, max_chars)
    }
    fn act(&self, element: &ElementRef, action: &UiaAction) -> Result<UiaNode, GuiError> {
        self.get().act(element, action)
    }
    fn password_rects(&self, window: WindowId) -> Result<Vec<ScreenRect>, GuiError> {
        self.get().password_rects(window)
    }
    fn focused(&self, window: WindowId) -> Result<Option<UiaNode>, GuiError> {
        self.get().focused(window)
    }
}

impl InputPort for LazyWinGui {
    fn send(&self, plan: &InputPlan, control: &InputControl) -> Result<InputReport, GuiError> {
        self.get().send(plan, control)
    }
}

impl ScreenCapturePort for LazyWinGui {
    fn capture(&self, request: &CaptureRequest) -> Result<Screenshot, GuiError> {
        self.get().capture(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descendants_follow_the_whole_tree() {
        let parents: BTreeMap<u32, u32> = [(10, 1), (11, 10), (12, 11), (20, 2), (13, 10)].into();
        let d = descendants(1, &parents);
        assert_eq!(d, [10, 11, 12, 13].into());
        assert!(descendants(99, &parents).is_empty());
    }

    #[test]
    fn guard_protects_alfa_dirs_and_this_process() {
        let dir = Path::new(r"C:\Users\ala\AppData\Local\Alfa");
        let g = alfa_guard(dir);
        assert!(g.is_protected(std::process::id(), "notepad.exe"));
        assert!(g.is_protected(7, r"C:\Users\ala\AppData\Local\Alfa\versions\1\x.exe"));
        assert!(g.is_protected(7, "alfa-broker-ui.exe"));
        assert!(!g.is_protected(7, r"C:\Windows\notepad.exe"));
    }

    #[test]
    fn lazy_ports_are_unsupported_off_windows() {
        let gui = LazyWinGui::new(Path::new("/tmp/alfa"));
        if !cfg!(windows) {
            assert!(gui.windows().is_err());
        }
        assert!(gui.guard().is_protected(std::process::id(), "x.exe"));
    }
}
