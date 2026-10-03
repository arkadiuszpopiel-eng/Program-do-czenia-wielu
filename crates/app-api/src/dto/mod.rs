//! DTO komend i zdarzeń IPC — kształt 1:1 z `apps/desktop/ui/src/lib/api/types*.ts`
//! (pola `snake_case`, jak serializuje serde). Test round-trip: `tests/dto_roundtrip.rs`.
//!
//! Znaczenie pól i wariantów jest opisane w `types*.ts` (kontrakt UI, docelowo generowany z tych
//! typów — ADR 0013); tu dokumentujemy typy, a nie powtarzamy opisów każdego pola.

#![allow(missing_docs)]

mod agents;
mod bridges;
mod common;
mod events;
mod gui;
mod health;
mod hub;
mod memory;
mod panels;
mod sessions;
mod skills;
mod system;
mod tasks;
mod transfer;
mod updates;

pub use agents::{
    AgentRun, AgentRunDetail, IntentKind, ReplayKind, ReplayStatus, ReplayStep, RunBudgetView,
    RunState, RunUsage, SessionWorkdir, ToolIntent, VoiceMode, VoiceSpeaker, VoiceState,
    VoiceStatus, WorkdirChoice,
};
pub use bridges::{BridgeCard, BridgeLogin, BridgeSource};
pub use common::{
    AutonomyLevel, Currency, Iso8601, Locale, LocalizedText, ModelProfile, Money, iso,
};
pub use events::{AlfaEvent, StopReason, ToastKind};
pub use gui::{
    GuiAction, GuiActionStatus, GuiControl, GuiScreenshot, GuiShotInfo, GuiStatus, TerminalFrame,
    TerminalProfileId, TerminalSession,
};
pub use health::{
    EvalSuiteView, EvalVerdictView, EvalsView, HealthHumanAction, HealthIncident, HealthModule,
    HealthOverall, HealthProposal, HealthRepair, HealthView, ImproverBlocked, ImproverChange,
    ImproverIssue, ImproverProposalView, ImproverView, ModuleHealth, RiskView,
};
pub use hub::{
    Account, AccountAssignment, AccountCostLimit, AccountState, AddAccountInput, AudioDevice,
    AuthKind, BatteryView, BrokerIntentResult, BrokerIntentStatus, CompatKind, ComplianceStatus,
    CpuView, DeviceProfile, GpuView, HwClass, LocalDownloadState, LocalModelInfo, MachineView,
    ModelInfo, ModelKind, PermissionsState, ProviderInfo, ProviderKind, RecommendationView,
    SecretInput, TestReport, VoiceProfileId,
};
pub use memory::{
    ConsolidationReport, MemoryCascadeItem, MemoryEdit, MemoryExplanation, MemoryForgetPreview,
    MemoryForgetReport, MemoryForgetTarget, MemoryItem, MemoryJournalEntry, MemoryLayer,
    MemoryPage, MemoryQuery, MemoryScopeInfo, MemoryScopeKind, MemoryScopeRef, MemorySourceKind,
    MemorySourceLink, MemoryState, MemoryStatus, MemoryUndoResult,
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
pub use skills::{
    AgentDraft, BuilderAgentInfo, BuilderDryRun, BuilderDryStep, BuilderPolicyView, BuilderPreview,
    BuilderProposal, BuilderSaved, DiffKind, DiffLine, DryOutcome, SkillImportResult, SkillInfo,
    SkillOrigin, SkillReview, SkillStateView,
};
pub use system::{
    AppBootstrap, DiskInfo, LayoutPrefs, MicAvailability, MicState, PanelId, QuickAskResult,
    RateLimitInfo, SelectOption, SessionPanels, SettingControl, SettingDef, SettingScope,
    SettingValue, SettingsCustomPage, SettingsPageDef, SystemStatus, VoicePillState,
};
pub use tasks::{
    CronPreview, MarshalProposalInfo, MarshalRejected, MarshalReport, MarshalRuleInfo,
    MarshalState, NewTaskInput, TaskClassKind, TaskDep, TaskInfo, TaskOriginKind, TaskResultKind,
    TaskStateKind, TriggerDraft, TriggerInfo, TriggerKindView, TriggerRunInfo,
};
pub use transfer::{
    CollisionResolution, DryRunItem, DryRunKind, ExportRequest, ExportResult, ExportScope,
    ImportMode, ImportRequest, ImportResult, InspectResult, ItemDiff, PackageManifestSummary,
};
pub use updates::{
    AboutInfo, LicenseEntry, LicenseSource, UpdateChannel, UpdateMode, UpdatePhase, UpdateProgress,
    UpdateRelease, UpdatesView, WhatsNew,
};
