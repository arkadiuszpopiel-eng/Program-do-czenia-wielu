// Typy DTO warstwy danych UI (IPC Tauri ↔ Svelte). Kształt danych łącza pochodzi wyłącznie z typów
// Rust (`generated/ipc.ts`, ADR 0013, B.1 etap 2) — tu jest tylko przekazanie i typy samego interfejsu.
// Pola w snake_case — tak jak serializuje je serde po stronie Rust.
export type {
  ActivityInfo,
  AgentState,
  AgentStatus,
  ApprovalPending,
  ArtifactAction,
  ArtifactInfo,
  ArtifactPreview,
  Author,
  AutonomyLevel,
  CastTemplateId,
  CostSummary,
  EventLevel,
  Locale,
  LocalizedText,
  ModelProfile,
  Money,
  PipelineDef,
  PipelineStage,
  PriceChangeRow,
  PriceRow,
  PricingView,
  ProjectInfo,
  ProjectRef,
  RememberScope,
  RenderedBlock,
  SendOptions,
  SendResult,
  SessionCard,
  SessionSearchHit,
  SessionSummary,
  SessionTemplate,
  TimelineEvent,
  TimelineFilter,
  TimelineKind,
  ToolIcon,
  ToolStep,
  Turn,
  TurnAnnotation,
  TurnError,
  TurnErrorCode,
  TurnStatus,
  TurnUsage,
  TurnsSnapshot,
  UndoTicket,
} from './generated/ipc';

/** Data i czas w RFC 3339 (UTC). */
export type Iso8601 = string;

export type * from './types-agents';
export type * from './types-hub';
export type * from './types-system';
