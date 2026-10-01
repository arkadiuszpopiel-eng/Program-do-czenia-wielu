//! Kontrakt `SystemPort` (docs/modules/platform-windows/SPEC.md) — neutralny wobec OS.
//!
//! Kod specyficzny dla Windows żyje wyłącznie w `platform-windows-impl`; tu są traity, typy
//! i reguły (odwracalność operacji, deny-lista poświadczeń, reguła AltGr dla skrótów).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod clipboard;
mod error;
mod exec;
mod fs;
mod hardware;
mod host;
mod hotkey;
mod input;
mod media;
mod peer;
mod pipe;
mod process;
mod surface;
mod tray;
mod window;

pub use clipboard::{ClipboardContent, ClipboardPort};
pub use error::PlatformError;
pub use exec::{
    CapturedStream, DEFAULT_ENV_ALLOWLIST, ExecControl, ExecOutput, ExecPort, ExecSpec,
    ExecTermination, MAX_EXEC_OUTPUT_BYTES, MAX_EXEC_TIMEOUT_MS, filter_env, is_secret_env_name,
};
pub use fs::{
    DirEntry, FsOperation, FsPort, KnownFolder, OpReceipt, UndoToken, is_credential_path,
};
pub use hardware::{
    AudioDirection, AudioEndpoint, CpuSummary, GpuAdapter, HardwarePort, OsSummary, PowerStatus,
};
pub use hotkey::{Hotkey, HotkeyEvent, HotkeyId, HotkeyPort, KILL_SWITCH, Key, Modifiers};
pub use process::{Integrity, ProcessHandle, ProcessPort, ProcessSpec, ProcessStatus};
pub use tray::{Notification, TrayMenuItem, TrayPort, TrayState};
pub use window::{WindowId, WindowInfo, WindowPort};
// Porty Jądra bezpieczeństwa (F3, część 2): potok z ACL, tożsamość klienta, okno Broker-UI,
// wejście wstrzyknięte, usługa i uruchamianie w sesji użytkownika, MMCSS, wolne miejsce.
pub use host::{
    LaunchIntegrity, PrivateDirPort, ServiceBody, ServiceHostPort, SessionLaunch,
    SessionLauncherPort, StopSignal,
};
pub use input::{
    HOOK_WINDOW_MS, HookObservation, HookOrigin, InputDevice, InputSample, LLKHF_INJECTED,
    LLKHF_LOWER_IL_INJECTED, LLMHF_INJECTED, LLMHF_LOWER_IL_INJECTED, MessageOrigin,
    input_is_injected,
};
pub use media::{DiskPort, DiskSpace, MmcssPort, MmcssTask, ThreadBoost};
pub use peer::{
    CodeSignaturePort, IntegrityLevel, PeerIdentity, PeerRejection, PeerRequirement,
    ProcessIdentityPort, Sid, SignatureStatus, UnverifiedSignatures, same_image,
};
pub use pipe::{
    CLIENT_ACCESS_MASK, PIPE_PREFIX, PipeConnection, PipeListener, PipeSecurity, SecurePipePort,
    private_dir_sddl, validate_pipe_name,
};
pub use surface::{
    ApprovalSurfacePort, MAX_BUTTONS, MAX_DETAILS, MAX_TEXT_CHARS, MIN_BUTTON_ID, PixelRect,
    SurfaceButton, SurfaceEvent, SurfaceLayout, SurfaceTone, SurfaceView,
};

/// Pełny port systemowy: suma sub-portów. Implementowany automatycznie przez każdy typ,
/// który implementuje wszystkie sub-traity. `HardwarePort` jest osobno (tylko `device-profile`).
pub trait SystemPort:
    FsPort + ProcessPort + ClipboardPort + WindowPort + HotkeyPort + TrayPort + Send + Sync
{
}

impl<T> SystemPort for T where
    T: FsPort + ProcessPort + ClipboardPort + WindowPort + HotkeyPort + TrayPort + Send + Sync
{
}
