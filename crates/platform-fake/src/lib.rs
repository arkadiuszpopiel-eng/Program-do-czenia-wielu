//! Atrapa `SystemPort` do testów (docs/modules/platform-windows/SPEC.md, sekcja „Fake”).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod clipboard;
mod exec;
mod fs;
mod hotkeys;
mod kernel_host;
mod misc;
mod pipes;

use std::path::{Path, PathBuf};

use platform_contract::{
    ClipboardContent, ClipboardPort, DirEntry, FsPort, Hotkey, HotkeyEvent, HotkeyId, HotkeyPort,
    KnownFolder, Notification, OpReceipt, PlatformError, ProcessHandle, ProcessPort, ProcessSpec,
    ProcessStatus, TrayMenuItem, TrayPort, TrayState, UndoToken, WindowId, WindowInfo, WindowPort,
};

pub use clipboard::FakeClipboard;
pub use exec::{ExecEffect, FakeExec, FakeRun, command_text};
pub use fs::{FakeFs, FsSnapshot};
pub use hotkeys::FakeHotkeys;
pub use misc::{FakeProcesses, FakeTray, FakeWindows};
// Porty Jądra bezpieczeństwa (F3, część 2).
pub use kernel_host::{
    FakeDisk, FakeLauncher, FakeMmcss, FakePrivateDirs, FakeServiceHost, FakeSurface,
};
pub use pipes::{FakePipeConnection, FakePipeListener, FakePipes};

/// Pełna atrapa systemu: składa wszystkie fake'i w jeden `SystemPort`.
#[derive(Debug, Default)]
pub struct FakePlatform {
    /// Wirtualny system plików.
    pub fs: FakeFs,
    /// Schowek.
    pub clipboard: FakeClipboard,
    /// Skróty globalne.
    pub hotkeys: FakeHotkeys,
    /// Okna.
    pub windows: FakeWindows,
    /// Procesy.
    pub processes: FakeProcesses,
    /// Zasobnik.
    pub tray: FakeTray,
}

impl FsPort for FakePlatform {
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

impl ClipboardPort for FakePlatform {
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

impl HotkeyPort for FakePlatform {
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

impl WindowPort for FakePlatform {
    fn list(&self) -> Vec<WindowInfo> {
        self.windows.list()
    }
    fn focus(&self, id: WindowId) -> Result<(), PlatformError> {
        self.windows.focus(id)
    }
}

impl ProcessPort for FakePlatform {
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

impl TrayPort for FakePlatform {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_system_port<T: platform_contract::SystemPort>(_: &T) {}

    #[test]
    fn fake_platform_is_a_system_port() {
        let p = FakePlatform::default();
        assert_system_port(&p);
        assert_eq!(p.state(), TrayState::Idle);
    }
}
