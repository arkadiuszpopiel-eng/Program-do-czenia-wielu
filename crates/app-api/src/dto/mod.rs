//! DTO komend i zdarzeń IPC — kształt 1:1 z `apps/desktop/ui/src/lib/api/types*.ts`
//! (pola `snake_case`, jak serializuje serde). Test round-trip: `tests/dto_roundtrip.rs`.
//!
//! Znaczenie pól i wariantów jest opisane w `types*.ts` (kontrakt UI, docelowo generowany z tych
//! typów — ADR 0013); tu dokumentujemy typy, a nie powtarzamy opisów każdego pola.

#![allow(missing_docs)]

mod advanced;
mod agents;
mod board;
mod bridges;
mod broker;
mod cameras;
mod common;
mod diag;
mod events;
mod files;
mod gpu_live;
mod gui;
mod health;
mod help;
mod hub;
mod licenses;
mod mcp;
mod memory;
mod memory_teach;
mod model_queues;
mod model_routing;
mod models;
mod modules;
mod panels;
mod plugins;
mod quick;
mod routes;
mod sessions;
mod skills;
mod system;
mod tasks;
mod templates;
mod transfer;
mod updates;
mod voice_engines;
mod voice_features;

pub use advanced::RawConfig;
pub use agents::{
    AgentRun, AgentRunDetail, IntentKind, ReplayKind, ReplayStatus, ReplayStep, RunBudgetView,
    RunState, RunUsage, SessionWorkdir, ToolIntent, VoiceMode, VoiceSpeaker, VoiceState,
    VoiceStatus, WorkdirChoice,
};
pub use board::BoardItem;
pub use bridges::{BridgeAccount, BridgeAccountState, BridgeCard, BridgeLogin, BridgeSource};
pub use broker::{BrokerLinkState, BrokerMode, BrokerStatusView};
pub use cameras::{
    CAMERA_FRAME_MAX_SIDE, CAMERA_FRAME_MIN_SIDE, CameraFrame, CameraKindView, CameraStateView,
    CameraView, CamerasState,
};
pub use common::{
    AutonomyLevel, Currency, Iso8601, Locale, LocalizedText, ModelProfile, Money, iso,
};
pub use diag::{
    BatteryDiag, CheckKind, CheckResult, CheckRun, CpuDiag, DiagProgress, DiagView, DiskDiag,
    EventCount, EventKind, EventLine, EventSummary, GpuDiag, Health, MachineDiag, MediaKind,
    MemDiag, MemModule, Metric, NetDiag, Snapshot, ThermalDiag, Verdict, VolumeDiag,
};
pub use events::{AlfaEvent, EventFrame, EventsResume, StopReason, ToastKind};
pub use files::{
    AttachmentDelivery, AttachmentInfo, AttachmentKind, AttachmentRejectReason,
    AttachmentRejection, AttachmentsAdded, BackupCheck, BackupConfig, BackupEntry, BackupView,
    CanvasKind, ConversationFormat, TurnAttachment,
};
pub use gpu_live::{GpuLive, GpuLiveCard, GpuLiveProcess};
pub use gui::{
    GuiAction, GuiActionStatus, GuiControl, GuiScreenshot, GuiShotInfo, GuiStatus, TerminalFrame,
    TerminalProfileId, TerminalSession,
};
pub use health::{
    EvalSuiteView, EvalVerdictView, EvalsView, HealthHumanAction, HealthIncident, HealthModule,
    HealthOverall, HealthProposal, HealthRepair, HealthView, ImproverBlocked, ImproverChange,
    ImproverIssue, ImproverProposalView, ImproverView, ModuleHealth, RiskView,
};
pub use help::{HelpHit, HelpPage, HelpTopic};
pub use hub::{
    Account, AccountAssignment, AccountCostLimit, AccountLastTest, AccountState, AddAccountInput,
    AudioDevice, AuthKind, AzureDeployment, BatteryView, BrokerIntentResult, BrokerIntentStatus,
    CompatKind, ComplianceStatus, CpuView, DeviceProfile, EnterpriseSettings, EnvImportKind,
    EnvImportResult, GpuView, HwClass, LocalDownloadState, LocalModelInfo, MachineView, ModelInfo,
    ModelKind, PermissionsState, ProviderInfo, ProviderKind, RecommendationView, SecretInput,
    TestReport, VoiceProfileId,
};
pub use licenses::{CommercialUse, LicenseCard, VoiceLicense};
pub use mcp::{
    McpConnState, McpEnvVar, McpServerInput, McpServerTools, McpServerView, McpToolState,
    McpToolView, McpTrust,
};
pub use memory::{
    ConsolidationReport, MemoryCascadeItem, MemoryEdit, MemoryExplanation, MemoryForgetPreview,
    MemoryForgetReport, MemoryForgetTarget, MemoryItem, MemoryJournalEntry, MemoryLayer,
    MemoryPage, MemoryQuery, MemoryScopeInfo, MemoryScopeKind, MemoryScopeRef, MemorySourceKind,
    MemorySourceLink, MemoryState, MemoryStatus, MemoryUndoResult,
};
pub use memory_teach::{TeachFile, TeachReport, TeachRequest, TeachSource};
pub use model_queues::{SharedQueueEntry, SharedQueues};
pub use model_routing::{
    ModelRouting, ModelRoutingSettings, RoutingLevel, RoutingModel, RoutingPreview, RoutingRung,
    RoutingStrategy,
};
pub use models::{
    BundleFit, BundleFitKind, BundleItemView, BundleRequirements, BundleState, EmbedderView,
    ModelBundle, ModelCard, ModelFileView, ModelItem, ModelItemKind, ModelItemState,
    ModelProgressView, ModelsView, QualityNote, ReindexView, TrustedHashes,
};
pub use modules::{ModuleRow, ModulesView};
pub use panels::{
    ActivityInfo, AgentState, AgentStatus, ArtifactAction, ArtifactInfo, ArtifactPreview,
    CastTemplateId, ContextUsage, CostLimitView, CostSummary, EventLevel, FxView, TimelineEvent,
    TimelineFilter, TimelineKind,
};
pub use plugins::{
    PluginCapabilityView, PluginInfo, PluginInspection, PluginLimitsView, PluginOrigin,
    PluginProblem, PluginProblemKind, PluginR2View, PluginStateView, PluginToolView, PluginsView,
};
pub use quick::{
    BuiltinCommand, CustomCommand, QuickCommandsView, QuickMatchMode, QuickSource, QuickTest,
};
pub use routes::{DataRoute, DataRoutesView};
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
pub use templates::PromptTemplate;
pub use transfer::{
    CollisionResolution, DryRunItem, DryRunKind, ExportRequest, ExportResult, ExportScope,
    ImportMode, ImportRequest, ImportResult, InspectResult, ItemDiff, PackageManifestSummary,
};
pub use updates::{
    AboutInfo, ChangeItem, ChangeLogEntry, ChangeSeverity, LicenseEntry, LicenseSource,
    UpdateChannel, UpdateMode, UpdatePhase, UpdateProgress, UpdateRelease, UpdatesView, WhatsNew,
};
pub use voice_engines::{
    BreakerStateView, EngineCheckView, EnginesAction, SelfTestView, SttEngineKind, SttEngineView,
    SttGuardView, TtsEngineView, TurnEngineKind, TurnEngineView, TurnStatsView, VoiceEnginesView,
};
pub use voice_features::{
    DictationAction, DictationProfile, DictationStateView, DictationView, EnrollSampleView,
    ReadAction, ReadAloudView, ReadControlAction, ReadSource, ReadStateView, S2sView,
    SampleQuality, SpeakerAction, SpeakerCheckView, SpeakerDecisionView, SpeakerState, SpeakerView,
    VoiceFeatures, WakeAction, WakeCalibrationView, WakeTestView, WakeWordsState, WakeWordsView,
};
