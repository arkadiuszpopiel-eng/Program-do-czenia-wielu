// DTO: Hub kont i kluczy (accounts-hub), import/eksport `.alfa` (transfer), uprawnienia, urządzenia.
import type { GpuView, SharedQueueEntry } from './generated/ipc';
export type {
  Account,
  AccountAssignment,
  AccountLastTest,
  AccountState,
  AddAccountInput,
  AudioDevice,
  AzureDeployment,
  BrokerIntentResult,
  CollisionResolution,
  GrantView,
  ComplianceStatus,
  DeviceProfile,
  DryRunItem,
  EnterpriseSettings,
  EnvImportResult,
  ExportRequest,
  ExportResult,
  ExportScope,
  GpuLive,
  GpuLiveCard,
  GpuLiveProcess,
  HwClass,
  ImportMode,
  ImportRequest,
  ImportResult,
  InspectResult,
  ItemDiff,
  LocalDownloadState,
  LocalModelInfo,
  ModelInfo,
  ModelPurpose,
  ModelRouting,
  ModelRoutingSettings,
  ModelTier,
  PackageManifestSummary,
  PermissionsState,
  ProviderInfo,
  ProviderKind,
  RoutingLevel,
  RoutingModel,
  RoutingPreview,
  RoutingRung,
  RoutingStrategy,
  SharedQueueEntry,
  TestReport,
  VoiceProfileId,
} from './generated/ipc';

// ── Konta i klucze (PLAN §5.6) ──────────────────────────────────────────────────────────────────

/** Klasa zadań (`chat`, `code`, …) → wspólna kolejka w kolejności prób Routera. */
export type SharedQueues = Readonly<Record<string, readonly SharedQueueEntry[]>>;

// ── Urządzenia (device-profile) ─────────────────────────────────────────────────────────────────

/** Karta z profilu urządzenia (w Rust: `GpuView`; rola zawężona w generatorze). */
export type GpuInfo = GpuView;
