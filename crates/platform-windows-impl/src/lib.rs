//! Implementacja `SystemPort` i `HardwarePort` dla Windows (docs/modules/platform-windows/SPEC.md).
//!
//! Jedyny crate workspace z windows-rs/COM (PLAN §3.2). Zakres F1: pliki (Kosz, dziennik cofnięć,
//! deny-lista poświadczeń), procesy w Job Object, schowek, okna, skróty globalne + hook PTT,
//! adapter zasobnika, sonda sprzętu. UIA i SendInput dochodzą w F5/F6.
//!
//! Wątki: skróty i hook — dedykowany wątek z pętlą komunikatów; `IFileOperation` — krótkotrwały
//! wątek STA; MMDevice — krótkotrwały wątek MTA; schowek i okna — wątek wywołującego (bez COM).
//! Poza Windows crate się kompiluje: FS działa przenośnie (bez Kosza), reszta zwraca `Unsupported`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
// Poza Windows część kodu (mapowania, kodery, stałe Win32) jest używana tylko przez testy;
// martwy kod wyłapuje `cargo clippy --target x86_64-pc-windows-msvc`.
#![cfg_attr(not(windows), allow(dead_code))]

mod clipboard;
mod config;
mod error;
mod fs;
mod hardware;
mod hotkey;
mod process;
mod tray;
#[cfg(windows)]
mod win;
mod window;

use std::path::{Path, PathBuf};

use platform_contract::{
    AudioEndpoint, ClipboardContent, ClipboardPort, CpuSummary, DirEntry, FsPort, GpuAdapter,
    HardwarePort, Hotkey, HotkeyEvent, HotkeyId, HotkeyPort, KnownFolder, Notification, OpReceipt,
    OsSummary, PlatformError, PowerStatus, ProcessHandle, ProcessPort, ProcessSpec, ProcessStatus,
    TrayMenuItem, TrayPort, TrayState, UndoToken, WindowId, WindowInfo, WindowPort,
};

pub use clipboard::{ClipboardConfig, WinClipboard};
pub use config::PlatformConfig;
pub use fs::{DEFAULT_EXTRA_DENY_NAMES, DEFAULT_EXTRA_DENY_PREFIXES, FsConfig, WinFs};
pub use hardware::WinHardware;
pub use hotkey::WinHotkeys;
pub use process::{JobLimits, ProcessInfo, WinProcesses, affinity_mask_for};
pub use tray::{TrayAdapter, TrayBackend};
pub use window::{DEFAULT_PROTECTED_PROCESSES, Rect, WinWindows, WindowDetails, WindowGuard};

/// Treść `module.toml` modułu `platform-windows`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Pełny port systemowy Windows. Pola są publiczne, żeby kompozycja (jądro, watchdog) miała
/// dostęp do metod spoza kontraktu (`list_detailed`, `spawn_with_limits`, `register_kill_switch`…).
#[derive(Debug)]
pub struct WindowsPlatform {
    /// Pliki.
    pub fs: WinFs,
    /// Procesy.
    pub processes: WinProcesses,
    /// Schowek.
    pub clipboard: WinClipboard,
    /// Okna.
    pub windows: WinWindows,
    /// Skróty globalne.
    pub hotkeys: WinHotkeys,
    /// Zasobnik (adapter).
    pub tray: TrayAdapter,
    /// Sprzęt.
    pub hardware: WinHardware,
}

impl WindowsPlatform {
    /// Składa port z konfiguracji.
    pub fn new(config: PlatformConfig) -> Self {
        let fs = WinFs::new(config.fs);
        let clipboard = WinClipboard::new(config.clipboard, fs.policy().clone());
        Self {
            fs,
            processes: WinProcesses::new(config.job_defaults),
            clipboard,
            windows: WinWindows::new(config.windows),
            hotkeys: WinHotkeys::new(),
            tray: TrayAdapter::default(),
            hardware: WinHardware,
        }
    }
}

impl Default for WindowsPlatform {
    fn default() -> Self {
        Self::new(PlatformConfig::default())
    }
}

impl FsPort for WindowsPlatform {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PlatformError> {
        self.fs.read(path)
    }
    fn write_atomic(&self, path: &Path, data: &[u8]) -> Result<OpReceipt, PlatformError> {
        self.fs.write_atomic(path, data)
    }
    fn copy(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError> {
        self.fs.copy(from, to)
    }
    fn move_path(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError> {
        self.fs.move_path(from, to)
    }
    fn delete_to_recycle_bin(&self, path: &Path) -> Result<OpReceipt, PlatformError> {
        self.fs.delete_to_recycle_bin(path)
    }
    fn delete_permanent(&self, path: &Path) -> Result<OpReceipt, PlatformError> {
        self.fs.delete_permanent(path)
    }
    fn exists(&self, path: &Path) -> bool {
        self.fs.exists(path)
    }
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        self.fs.list_dir(path)
    }
    fn undo(&self, token: UndoToken) -> Result<(), PlatformError> {
        self.fs.undo(token)
    }
    fn known_folder(&self, folder: KnownFolder) -> PathBuf {
        self.fs.known_folder(folder)
    }
}

impl ProcessPort for WindowsPlatform {
    fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, PlatformError> {
        self.processes.spawn(spec)
    }
    fn kill_tree(&self, handle: ProcessHandle) -> Result<(), PlatformError> {
        self.processes.kill_tree(handle)
    }
    fn status(&self, handle: ProcessHandle) -> Result<ProcessStatus, PlatformError> {
        self.processes.status(handle)
    }
    fn foreground_is_elevated(&self) -> bool {
        self.processes.foreground_is_elevated()
    }
}

impl ClipboardPort for WindowsPlatform {
    fn get(&self) -> Result<ClipboardContent, PlatformError> {
        self.clipboard.get()
    }
    fn set(&self, content: ClipboardContent) -> Result<(), PlatformError> {
        self.clipboard.set(content)
    }
    fn restore_previous(&self) -> Result<bool, PlatformError> {
        self.clipboard.restore_previous()
    }
}

impl WindowPort for WindowsPlatform {
    fn list(&self) -> Vec<WindowInfo> {
        self.windows.list()
    }
    fn focus(&self, id: WindowId) -> Result<(), PlatformError> {
        self.windows.focus(id)
    }
}

impl HotkeyPort for WindowsPlatform {
    fn register(&self, hotkey: Hotkey) -> Result<HotkeyId, PlatformError> {
        self.hotkeys.register(hotkey)
    }
    fn unregister(&self, id: HotkeyId) -> Result<(), PlatformError> {
        self.hotkeys.unregister(id)
    }
    fn drain_events(&self) -> Vec<HotkeyEvent> {
        self.hotkeys.drain_events()
    }
}

impl TrayPort for WindowsPlatform {
    fn set_state(&self, state: TrayState) -> Result<(), PlatformError> {
        self.tray.set_state(state)
    }
    fn state(&self) -> TrayState {
        self.tray.state()
    }
    fn set_menu(&self, items: Vec<TrayMenuItem>) -> Result<(), PlatformError> {
        self.tray.set_menu(items)
    }
    fn notify(&self, notification: Notification) -> Result<(), PlatformError> {
        self.tray.notify(notification)
    }
}

impl HardwarePort for WindowsPlatform {
    fn os(&self) -> Result<OsSummary, PlatformError> {
        self.hardware.os()
    }
    fn cpu(&self) -> Result<CpuSummary, PlatformError> {
        self.hardware.cpu()
    }
    fn memory_total_mb(&self) -> Result<u64, PlatformError> {
        self.hardware.memory_total_mb()
    }
    fn gpus(&self) -> Result<Vec<GpuAdapter>, PlatformError> {
        self.hardware.gpus()
    }
    fn npu(&self) -> Result<Option<String>, PlatformError> {
        self.hardware.npu()
    }
    fn power_status(&self) -> Result<PowerStatus, PlatformError> {
        self.hardware.power_status()
    }
    fn audio_endpoints(&self) -> Result<Vec<AudioEndpoint>, PlatformError> {
        self.hardware.audio_endpoints()
    }
    fn machine_seed(&self) -> Result<Option<String>, PlatformError> {
        self.hardware.machine_seed()
    }
}
