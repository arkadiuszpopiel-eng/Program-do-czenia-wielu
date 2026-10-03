//! Kontrakt `SystemPort` (docs/modules/platform-windows/SPEC.md) — neutralny wobec OS.
//!
//! Kod specyficzny dla Windows żyje wyłącznie w `platform-windows-impl`; tu są traity, typy
//! i reguły (odwracalność operacji, deny-lista poświadczeń, reguła AltGr dla skrótów).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod capture;
mod capture_check;
mod clipboard;
mod desktop;
mod dirwatch;
mod dirwatch_core;
mod dirwatch_policy;
mod dirwatch_set;
mod error;
mod exec;
mod focus;
mod fs;
mod gui;
mod hardware;
mod host;
mod hotkey;
mod idle;
mod image;
mod input;
mod keys;
mod media;
mod peer;
mod pipe;
mod power;
mod presence;
mod process;
mod pty;
mod signals;
mod surface;
mod synth;
mod synth_plan;
mod target;
mod tray;
mod uia;
mod uia_action;
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
pub use hotkey::{
    Hotkey, HotkeyEvent, HotkeyId, HotkeyPort, HotkeyPressOrigin, KILL_SWITCH, Key, Modifiers,
};
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

// F6 (computer use) i F4 (terminal): okna v2, UI Automation, wejście syntetyczne, zrzuty
// z maskowaniem, strażnik celów (zakaz wobec okien Alfy/Brokera), pseudokonsola ConPTY.
pub use capture::{
    CAPTURE_SIDE_RANGE, CaptureRequest, CaptureTarget, DEFAULT_CAPTURE_MAX_SIDE,
    DEFAULT_MASKED_APPS, MaskReason, MaskedArea, ScreenCapturePort, Screenshot, finish_capture,
    mask_and_scale, mask_plan,
};
// Przegląd #2, P2-02: ponowne wyliczenie okien po klatce (TOCTOU maskowania).
pub use capture_check::{CAPTURE_ATTEMPTS, capture_set_stable, union_for_mask, unstable_masks};
pub use desktop::{
    DesktopPort, DesktopWindow, MIN_WINDOW_SIZE, MonitorInfo, WindowState, validate_bounds,
};
pub use gui::{GuiError, PROTECTED_IMAGES, ScreenRect, TargetGuard, image_file_name};
pub use image::{
    MASK_COLOR, MAX_IMAGE_PIXELS, RgbaImage, encode_png, encode_png_with, zlib_stored,
};
pub use keys::{ChordKey, KeyChord, is_extended_vk};
pub use pty::{PseudoConsolePort, PtySession, PtySize, PtySpec};
pub use synth::{
    Aim, InputBackend, InputBatch, InputControl, InputPacing, InputPlan, InputPort, InputReport,
    InputStep, MouseButton, RawInput, TargetWindow, execute as execute_input,
};
pub use synth_plan::plan_batches;
// Przegląd bezpieczeństwa #2: element z fokusem przed wpisywaniem (P2-03) i procesy powiązane
// z oknem-celem — WebView2 Alfy, okna-własności, UWP, drzewo procesów przy każdej akcji (P2-01).
pub use focus::{FocusedField, batch_writes_text};
pub use target::{
    LinkRole, MAX_ANCESTORS, ProcessLink, UWP_CORE_CLASS, UWP_FRAME_CLASS, UWP_FRAME_HOST,
    ancestors_of,
};
pub use uia::{
    ElementRef, ExpandState, MAX_SET_VALUE_CHARS, SPARSE_TREE_NODES, ScrollAmount, ScrollDirection,
    ToggleState, TreeOptions, UIA_CALL_TIMEOUT_MS, UIA_TREE_TIMEOUT_MS, UiaAction, UiaNode,
    UiaPattern, UiaPort, UiaQuery, UiaText, UiaTree, control_type_name,
};

// Sygnały systemowe i obserwacja katalogów (poza `SystemPort`): bezczynność z histerezą, zasilanie,
// tryb gry / pełny ekran, blokada sesji (`SignalMonitor`), `ReadDirectoryChangesW` z debounce,
// przeskanowaniem po przepełnieniu i deny-listą (`WatchSet`). Windows: `platform-windows-sys-impl`.
pub use dirwatch::{
    BASELINE_DENY_SEGMENTS, DEFAULT_DEBOUNCE_MS, DEFAULT_MAX_DELAY_MS, DEFAULT_MAX_WATCH_ENTRIES,
    DEFAULT_MAX_WATCHES, DirWatchPort, EVENT_FS_CHANGED, EVENT_FS_RESCANNED,
    EVENT_FS_WATCH_STOPPED, FsChangeKind, MAX_PATTERN_LEN, MAX_WATCH_PATTERNS, RescanReason,
    WatchEvent, WatchId, WatchSpec,
};
pub use dirwatch_core::{FileStamp, RawChange, WatchCore};
pub use dirwatch_policy::{WatchPolicy, glob_match, is_temp_name};
pub use dirwatch_set::WatchSet;
pub use idle::{
    DEFAULT_IDLE_AFTER_MS, DEFAULT_WAKE_CONFIRM_MS, DEFAULT_WAKE_WINDOW_MS, IdleConfig, IdlePort,
    IdleTracker, IdleTransition,
};
pub use power::{DEFAULT_PERCENT_STEP, LOW_BATTERY_PERCENT, PowerPort, PowerSnapshot, PowerSource};
pub use presence::{
    DEFAULT_GAME_EXIT_AFTER_MS, FullscreenPort, FullscreenProbe, GameModeTracker, GameReason,
    NotificationState, SessionPort, SessionState,
};
pub use signals::{
    DEFAULT_POLL_MS, EVENT_FULLSCREEN_CHANGED, EVENT_IDLE_ENTERED, EVENT_IDLE_EXITED,
    EVENT_POWER_CHANGED, EVENT_SESSION_LOCKED, EVENT_SESSION_UNLOCKED, MAX_QUEUED_SIGNALS,
    SignalConfig, SignalEvent, SignalMonitor, SignalSample, SystemSignals, SystemSignalsPort,
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
