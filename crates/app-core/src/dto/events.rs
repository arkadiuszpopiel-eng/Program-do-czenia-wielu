//! Zdarzenia `alfa://events` (paczka `AlfaEvent[]`, batch co klatkę) — odpowiednik `types-system.ts`.

use serde::{Deserialize, Serialize};

use super::common::LocalizedText;
use super::hub::Account;
use super::panels::{ActivityInfo, AgentState, CostSummary, TimelineEvent};
use super::sessions::{
    ApprovalPending, RenderedBlock, SessionSummary, ToolStep, Turn, TurnError, TurnStatus,
    TurnUsage,
};
use super::system::{SystemStatus, VoicePillState};

/// Powód zakończenia strumienia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    End,
    Refusal,
    ToolUse,
    MaxTokens,
    Cancelled,
}

/// Rodzaj komunikatu rdzenia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

/// Zdarzenie rdzeń → UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AlfaEvent {
    TurnAppended {
        session_id: String,
        turn: Box<Turn>,
    },
    TurnStatus {
        session_id: String,
        turn_id: String,
        status: TurnStatus,
    },
    TextDelta {
        session_id: String,
        turn_id: String,
        text: String,
        blocks: Vec<RenderedBlock>,
    },
    ThinkingDelta {
        session_id: String,
        turn_id: String,
        elapsed_ms: u64,
        done: bool,
    },
    ToolCall {
        session_id: String,
        turn_id: String,
        step: ToolStep,
    },
    ApprovalPending {
        session_id: String,
        turn_id: String,
        approval: ApprovalPending,
    },
    Usage {
        session_id: String,
        turn_id: String,
        usage: TurnUsage,
    },
    Stop {
        session_id: String,
        turn_id: String,
        reason: StopReason,
    },
    Error {
        session_id: String,
        turn_id: String,
        error: TurnError,
    },
    SessionUpdated {
        session: SessionSummary,
    },
    SessionRemoved {
        session_id: String,
    },
    AgentsChanged {
        session_id: String,
        agents: Vec<AgentState>,
    },
    ActivityChanged {
        session_id: String,
        activity: Option<ActivityInfo>,
    },
    CostsChanged {
        session_id: String,
        costs: CostSummary,
    },
    SystemStatusChanged {
        status: SystemStatus,
    },
    TimelineAppended {
        event: TimelineEvent,
    },
    AccountChanged {
        account: Account,
    },
    MicLevel {
        level: f64,
    },
    VoicePill {
        state: VoicePillState,
    },
    Toast {
        kind: ToastKind,
        message: LocalizedText,
    },
}
