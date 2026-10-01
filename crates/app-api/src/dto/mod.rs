//! DTO komend i zdarzeń IPC — kształt 1:1 z `apps/desktop/ui/src/lib/api/types*.ts`
//! (pola `snake_case`, jak serializuje serde). Test round-trip: `tests/dto_roundtrip.rs`.
//!
//! Znaczenie pól i wariantów jest opisane w `types*.ts` (kontrakt UI, docelowo generowany z tych
//! typów — ADR 0013); tu dokumentujemy typy, a nie powtarzamy opisów każdego pola.

#![allow(missing_docs)]

mod common;
mod events;
mod hub;
mod panels;
mod sessions;
mod system;
mod transfer;

pub use common::{
    AutonomyLevel, Currency, Iso8601, Locale, LocalizedText, ModelProfile, Money, iso,
};
pub use events::{AlfaEvent, StopReason, ToastKind};
pub use hub::{
    Account, AccountAssignment, AccountCostLimit, AccountState, AddAccountInput, AudioDevice,
    AuthKind, BatteryView, BrokerIntentResult, BrokerIntentStatus, CompatKind, ComplianceStatus,
    CpuView, DeviceProfile, GpuView, HwClass, LocalDownloadState, LocalModelInfo, MachineView,
    ModelInfo, ModelKind, PermissionsState, ProviderInfo, ProviderKind, RecommendationView,
    SecretInput, TestReport, VoiceProfileId,
};
pub use panels::{
    ActivityInfo, AgentState, AgentStatus, ArtifactAction, ArtifactInfo, ArtifactPreview,
    CastTemplateId, ContextUsage, CostLimitView, CostSummary, EventLevel, FxView, TimelineEvent,
    TimelineFilter, TimelineKind,
};
pub use sessions::{
    ApprovalPending, ApprovalStatus, BlockKind, ProjectRef, Rating, RememberScope, RenderedBlock,
    RiskLevel, SendOptions, SendResult, SessionSearchHit, SessionSummary, SessionTemplate,
    ThinkingInfo, ToolIcon, ToolStatus, ToolStep, Turn, TurnAnnotation, TurnError, TurnErrorCode,
    TurnStatus, TurnUsage, TurnsSnapshot, UndoTicket,
};
pub use system::{
    AppBootstrap, DiskInfo, LayoutPrefs, MicAvailability, MicState, PanelId, QuickAskResult,
    RateLimitInfo, SelectOption, SessionPanels, SettingControl, SettingDef, SettingScope,
    SettingValue, SettingsCustomPage, SettingsPageDef, SystemStatus, VoicePillState,
};
pub use transfer::{
    CollisionResolution, DryRunItem, DryRunKind, ExportRequest, ExportResult, ExportScope,
    ImportMode, ImportRequest, ImportResult, InspectResult, ItemDiff, PackageManifestSummary,
};
