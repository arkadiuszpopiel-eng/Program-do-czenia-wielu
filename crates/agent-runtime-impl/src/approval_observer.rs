//! Obserwacja oczekiwania na zgodę współdzielona przez rodzinę przebiegów.
use crate::handle::RunHandle;
use agent_runtime_contract::{RunEvent, StepStatus};
use safety_broker_contract::{ApprovalId, ApprovalTicket};
use std::sync::Arc;
use tools_common_contract::{ToolObserver, ToolStatus};
/// Obserwator wywołania: karta „czeka na zatwierdzenie” w UI.
pub(crate) struct StepObserver {
    pub(crate) handle: Arc<RunHandle>,
    pub(crate) step: u32,
    pub(crate) clock: Arc<crate::ApprovalClock>,
}

impl ToolObserver for StepObserver {
    fn approval_requested(&self, ticket: &ApprovalTicket) {
        self.clock.begin(format!("{:?}", ticket.id));
        self.handle.emit_detached(RunEvent::WaitingApproval {
            step: self.step,
            approval: ticket.id,
            explanation: ticket.explanation.clone(),
        });
    }

    fn approval_resolved(&self, id: ApprovalId, _approved: bool) {
        self.clock.end(&format!("{id:?}"));
        self.handle
            .emit_detached(RunEvent::ApprovalResolved { approval: id });
    }
}

pub(crate) fn step_status(s: &ToolStatus) -> StepStatus {
    match s {
        ToolStatus::Ok => StepStatus::Ok,
        ToolStatus::Denied { .. } => StepStatus::Denied,
        ToolStatus::NeedsConfirmation => StepStatus::NeedsConfirmation,
        ToolStatus::Failed { .. } => StepStatus::Failed,
        ToolStatus::Cancelled => StepStatus::Cancelled,
    }
}
