//! Katalog awarii chaosowych (po jednej na rodzaj katalogu Diagnosty) — zgodny z
//! `evals/F8/chaos/catalog.json`.

use diagnostician_contract::FailureKind;

/// Awaria w katalogu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultSpec {
    /// Identyfikator (jak w `catalog.json`).
    pub id: &'static str,
    /// Oczekiwana klasyfikacja.
    pub kind: FailureKind,
    /// Oczekiwany cel.
    pub target: &'static str,
}

const fn f(id: &'static str, kind: FailureKind, target: &'static str) -> FaultSpec {
    FaultSpec { id, kind, target }
}

/// Wszystkie awarie chaosowe (po jednej na rodzaj katalogu).
pub const FAULTS: [FaultSpec; 24] = [
    f(
        "f01-module-start",
        FailureKind::ModuleStartFailure,
        "voice-tts",
    ),
    f(
        "f02-config-corrupted",
        FailureKind::ConfigCorrupted,
        "config/shared.toml",
    ),
    f(
        "f03-session-db-corrupted",
        FailureKind::SessionDbCorrupted,
        "sesje/s1.db",
    ),
    f(
        "f04-session-db-locked",
        FailureKind::SessionDbLocked,
        "sesje/s2.db",
    ),
    f("f05-disk-full", FailureKind::DiskFull, "C:"),
    f(
        "f06-sidecar-crash-loop",
        FailureKind::SidecarCrashLoop,
        "pocket-tts",
    ),
    f("f07-gpu-lost", FailureKind::GpuLost, "voice-stt"),
    f(
        "f08-api-key-revoked",
        FailureKind::ApiKeyRevoked,
        "anthropic",
    ),
    f("f09-rate-limit-loop", FailureKind::RateLimitLoop, "openai"),
    f(
        "f10-budget-exhausted",
        FailureKind::BudgetExhausted,
        "miesiac",
    ),
    f("f11-port-in-use", FailureKind::PortInUse, "providers-local"),
    f(
        "f12-model-missing",
        FailureKind::ModelMissing,
        "bielik-4.5b",
    ),
    f(
        "f13-model-corrupted",
        FailureKind::ModelCorrupted,
        "models/whisper-small.bin",
    ),
    f(
        "f14-webview-broken",
        FailureKind::WebViewBroken,
        "webview/EBWebView",
    ),
    f(
        "f15-schema-mismatch",
        FailureKind::SchemaMismatch,
        "memory.db",
    ),
    f(
        "f16-undo-journal-full",
        FailureKind::UndoJournalFull,
        "undo-journal",
    ),
    f(
        "f17-file-locked",
        FailureKind::FileLocked,
        "config/machine/desktop.toml",
    ),
    f(
        "f18-dir-permission",
        FailureKind::DirPermissionDenied,
        "C:/Users/ja/Alfa/Sesje",
    ),
    f("f19-clock-skew", FailureKind::ClockSkew, "system"),
    f("f20-network-down", FailureKind::NetworkDown, "network"),
    f(
        "f21-update-package-corrupted",
        FailureKind::UpdatePackageCorrupted,
        "%LOCALAPPDATA%/Alfa/versions/0.0.2.zip",
    ),
    f(
        "f22-resource-budget",
        FailureKind::ResourceBudgetExceeded,
        "memory-consolidation",
    ),
    f(
        "f23-log-disk-limit",
        FailureKind::LogDiskLimit,
        "logs/diagnostics",
    ),
    f(
        "f24-kernel-config-tampered",
        FailureKind::KernelConfigTampered,
        "config/kernel.toml",
    ),
];
