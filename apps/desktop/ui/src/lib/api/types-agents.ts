// DTO agentek z narzędziami (Replay krok po kroku, intencje „uruchom w terminalu", katalog roboczy
// sesji) i trybu głosowego (stan potoku). Odpowiednik `crates/app-api/src/dto/agents.rs`.
export type {
  AgentRun,
  AgentRunDetail,
  IntentKind,
  ReplayKind,
  ReplayStatus,
  ReplayStep,
  RunBudgetView,
  RunState,
  RunUsage,
  SessionWorkdir,
  ToolIntent,
  VoiceMode,
  VoiceSpeaker,
  VoiceState,
  VoiceStatus,
  WorkdirChoice,
} from './generated/ipc';
