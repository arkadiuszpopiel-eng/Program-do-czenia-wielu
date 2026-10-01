//! Protokół Brokera dla narzędzi: `decide` → `Allow(token)` | `NeedsApproval` (czekanie na
//! decyzję w Broker-UI z limitem i anulowaniem) | `Deny`; potem `verify` przy każdym użyciu
//! i unieważnienie tokenu po akcji (token jednorazowy, krótkie życie).

use std::sync::Arc;
use std::time::Duration;

use safety_broker_contract::{
    ActionRequest, ApprovalId, ApprovalStatus, Broker, BrokerError, CapToken, Capability, Decision,
    DenyReason, Holder,
};

use crate::call::{DenialReason, ToolCtx, ToolErrorKind, ToolOutcome};

/// Domyślny odstęp sprawdzania stanu prośby o zatwierdzenie.
pub const DEFAULT_APPROVAL_POLL: Duration = Duration::from_millis(250);

/// Zgoda Brokera na jedną akcję.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorization {
    /// Token zdolności.
    pub token: CapToken,
    /// Prośba, przez którą przeszła zgoda (`None` = poziom autonomii).
    pub approval: Option<ApprovalId>,
}

/// Dlaczego nie ma zgody.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    /// Odmowa (Jądro, właściciel, wygaśnięcie, limit czasu, token).
    #[error("odmowa: {}", .0.describe())]
    Denied(DenialReason),
    /// Anulowano podczas czekania.
    #[error("anulowano")]
    Cancelled,
    /// Błąd Brokera (żądanie, IPC).
    #[error("Broker: {0}")]
    Broker(BrokerError),
}

impl GateError {
    /// Wynik narzędzia dla modelu.
    pub fn into_outcome(self, action: &str) -> ToolOutcome {
        match self {
            Self::Denied(reason) => ToolOutcome::denied(reason, action),
            Self::Cancelled => ToolOutcome::cancelled(action),
            Self::Broker(e) => ToolOutcome::failed(
                ToolErrorKind::Internal,
                format!("Nie wykonano: {action} — błąd Brokera: {e}."),
            ),
        }
    }
}

fn map_broker(e: BrokerError) -> GateError {
    match e {
        BrokerError::KernelBlock(rule) => GateError::Denied(DenialReason::KernelBlock { rule }),
        BrokerError::TokenRejected(_) | BrokerError::TokenExpired => {
            GateError::Denied(DenialReason::TokenRejected)
        }
        BrokerError::AuditUnavailable(_) => GateError::Denied(DenialReason::AuditUnavailable),
        other => GateError::Broker(other),
    }
}

/// Bramka Brokera współdzielona przez narzędzia.
#[derive(Clone)]
pub struct BrokerGate {
    broker: Arc<dyn Broker>,
    poll: Duration,
}

impl std::fmt::Debug for BrokerGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrokerGate")
            .field("poll", &self.poll)
            .finish_non_exhaustive()
    }
}

impl BrokerGate {
    /// Bramka z domyślnym odstępem sprawdzania.
    pub fn new(broker: Arc<dyn Broker>) -> Self {
        Self {
            broker,
            poll: DEFAULT_APPROVAL_POLL,
        }
    }

    /// Zmienia odstęp sprawdzania stanu prośby.
    #[must_use]
    pub fn with_poll(mut self, poll: Duration) -> Self {
        self.poll = poll.max(Duration::from_millis(1));
        self
    }

    /// Broker (np. do zgłaszania niezaufanej treści).
    pub fn broker(&self) -> &Arc<dyn Broker> {
        &self.broker
    }

    /// Zgoda na akcję: token od razu, po zatwierdzeniu w Broker-UI albo błąd z powodem.
    pub async fn authorize(
        &self,
        request: ActionRequest,
        ctx: &ToolCtx,
    ) -> Result<Authorization, GateError> {
        if ctx.cancel.is_cancelled() {
            return Err(GateError::Cancelled);
        }
        let holder = request.holder.clone();
        match self.broker.decide(request).await.map_err(map_broker)? {
            Decision::Allow(token) => Ok(Authorization {
                token,
                approval: None,
            }),
            Decision::Deny(DenyReason::KernelBlock(rule)) => {
                Err(GateError::Denied(DenialReason::KernelBlock { rule }))
            }
            Decision::Deny(DenyReason::AuditUnavailable) => {
                Err(GateError::Denied(DenialReason::AuditUnavailable))
            }
            Decision::NeedsApproval(ticket) => {
                if let Some(obs) = &ctx.observer {
                    obs.approval_requested(&ticket);
                }
                let result = self.wait(ticket.id, &holder, ctx).await;
                if let Some(obs) = &ctx.observer {
                    obs.approval_resolved(ticket.id, result.is_ok());
                }
                result
            }
        }
    }

    async fn wait(
        &self,
        id: ApprovalId,
        holder: &Holder,
        ctx: &ToolCtx,
    ) -> Result<Authorization, GateError> {
        let deadline = tokio::time::Instant::now() + ctx.approval_timeout;
        loop {
            match self
                .broker
                .approval_status(id, holder)
                .map_err(map_broker)?
            {
                ApprovalStatus::Pending => {}
                ApprovalStatus::Approved { token: Some(token) } => {
                    return Ok(Authorization {
                        token: *token,
                        approval: Some(id),
                    });
                }
                ApprovalStatus::Approved { token: None } => {
                    return Err(GateError::Broker(BrokerError::InvalidRequest(
                        "zatwierdzenie bez tokenu (token odebrany wcześniej)".into(),
                    )));
                }
                ApprovalStatus::Denied => {
                    return Err(GateError::Denied(DenialReason::OwnerDenied {
                        approval: id,
                    }));
                }
                ApprovalStatus::Expired => {
                    return Err(GateError::Denied(DenialReason::ApprovalExpired {
                        approval: id,
                    }));
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(GateError::Denied(DenialReason::ApprovalTimeout {
                    approval: id,
                }));
            }
            tokio::select! {
                () = ctx.cancel.cancelled() => return Err(GateError::Cancelled),
                () = tokio::time::sleep(self.poll) => {}
            }
        }
    }

    /// Zgody na kilka akcji po kolei; przy pierwszej odmowie unieważnia już wydane tokeny.
    pub async fn authorize_all(
        &self,
        requests: Vec<ActionRequest>,
        ctx: &ToolCtx,
    ) -> Result<Vec<Authorization>, GateError> {
        let mut granted = Vec::with_capacity(requests.len());
        for request in requests {
            match self.authorize(request, ctx).await {
                Ok(a) => granted.push(a),
                Err(e) => {
                    self.release(&granted).await;
                    return Err(e);
                }
            }
        }
        Ok(granted)
    }

    /// Weryfikacja tokenu dla konkretnego użycia (zakres, podmiot, MAC, TTL, reguły Jądra).
    pub fn verify(
        &self,
        auth: &Authorization,
        needed: &Capability,
        holder: &Holder,
    ) -> Result<(), GateError> {
        self.broker
            .verify(&auth.token, needed, holder)
            .map_err(map_broker)
    }

    /// Unieważnia tokeny po akcji (best effort — tokeny i tak mają TTL).
    pub async fn release(&self, auths: &[Authorization]) {
        for a in auths {
            let _ = self.broker.revoke(a.token.id).await;
        }
    }
}
