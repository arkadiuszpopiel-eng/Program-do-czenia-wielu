//! Kontrakt `SystemPort` (docs/modules/platform-windows/SPEC.md) — neutralny wobec OS.
//!
//! Kod specyficzny dla Windows żyje wyłącznie w `platform-windows-impl`; tu są traity, typy
//! i reguły (odwracalność operacji, deny-lista poświadczeń, reguła AltGr dla skrótów).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod clipboard;
mod error;
mod fs;
mod hotkey;
mod process;
mod tray;
mod window;

pub use clipboard::{ClipboardContent, ClipboardPort};
pub use error::PlatformError;
pub use fs::{
    is_credential_path, DirEntry, FsOperation, FsPort, KnownFolder, OpReceipt, UndoToken,
};
pub use hotkey::{Hotkey, HotkeyEvent, HotkeyId, HotkeyPort, Key, Modifiers, KILL_SWITCH};
pub use process::{Integrity, ProcessHandle, ProcessPort, ProcessSpec, ProcessStatus};
pub use tray::{Notification, TrayMenuItem, TrayPort, TrayState};
pub use window::{WindowId, WindowInfo, WindowPort};

/// Pełny port systemowy: suma sub-portów. Implementowany automatycznie przez każdy typ,
/// który implementuje wszystkie sub-traity.
pub trait SystemPort:
    FsPort + ProcessPort + ClipboardPort + WindowPort + HotkeyPort + TrayPort + Send + Sync
{
}

impl<T> SystemPort for T where
    T: FsPort + ProcessPort + ClipboardPort + WindowPort + HotkeyPort + TrayPort + Send + Sync
{
}
