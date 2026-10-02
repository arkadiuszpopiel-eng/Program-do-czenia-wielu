//! Komendy panelu „Ekran" (`gui_*`): stan, ostatni zrzut, przejęcie i oddanie sterowania,
//! „zawsze zezwalaj na podgląd pulpitu" — prośba do Brokera o `gui.control(desktop.exe)` dla
//! agentki w sesji; zakres „zawsze" (≤ 24 h, przycina Broker) wybiera właściciel wyłącznie w oknie
//! Brokera (Broker-UI), nigdy w WebView. Token jednorazowy z tej prośby jest unieważniany.

use std::sync::Arc;
use std::time::Duration;

use app_api::dto::{BrokerIntentResult, BrokerIntentStatus, GuiScreenshot, GuiStatus};
use app_api::ports::BrokerPort;
use app_api::{AppError, ErrorCode};
use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::CommandOrigin;
use safety_broker_contract::{
    ActionRequest, ApprovalId, ApprovalStatus, Broker, Capability, Decision, DeclaredFacts,
    DenyReason, Holder,
};
use tools_window_contract::gui::DESKTOP_APP;

use crate::monitor::GuiMonitor;

/// Jak długo czekać na decyzję w oknie Brokera, zanim przestaniemy pilnować tokenu prośby.
const GRANT_WATCH: Duration = Duration::from_secs(15 * 60);
/// Odstęp sprawdzania stanu prośby.
const GRANT_POLL: Duration = Duration::from_secs(1);

/// Panel „Ekran".
pub struct GuiApp {
    monitor: Arc<GuiMonitor>,
    broker: Option<Arc<dyn Broker>>,
    port: Arc<dyn BrokerPort>,
}

impl std::fmt::Debug for GuiApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiApp")
            .field("monitor", &self.monitor)
            .field("broker", &self.broker.is_some())
            .finish_non_exhaustive()
    }
}

fn valid_agent(agent: &str) -> bool {
    !agent.is_empty()
        && agent.len() <= 32
        && agent
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl GuiApp {
    /// Panel nad monitorem; `broker` — decyzje (bez niego prośba jest niedostępna), `port` —
    /// pokazanie karty w oknie Brokera.
    pub fn new(
        monitor: Arc<GuiMonitor>,
        broker: Option<Arc<dyn Broker>>,
        port: Arc<dyn BrokerPort>,
    ) -> Self {
        Self {
            monitor,
            broker,
            port,
        }
    }

    /// Monitor (narzędzia, testy).
    pub fn monitor(&self) -> &Arc<GuiMonitor> {
        &self.monitor
    }

    /// `gui_status`.
    pub fn status(&self) -> GuiStatus {
        self.monitor.status()
    }

    /// `gui_screenshot`: ostatni zrzut agentki (piksele tylko w pamięci procesu).
    pub fn screenshot(&self) -> Option<GuiScreenshot> {
        self.monitor.screenshot()
    }

    /// `gui_stop`: przejęcie — sesje, których przebiegi rdzeń ma zatrzymać.
    pub fn take_over(&self) -> Vec<SessionId> {
        self.monitor.take_over()
    }

    /// `gui_release`.
    pub fn release(&self) -> GuiStatus {
        self.monitor.release();
        self.monitor.status()
    }

    /// `gui_desktop_grant`: prośba do Brokera o podgląd pulpitu dla agentki w sesji.
    pub async fn desktop_grant(
        &self,
        session: SessionId,
        agent: &str,
    ) -> Result<BrokerIntentResult, AppError> {
        if !valid_agent(agent) {
            return Err(AppError::invalid(format!("Nieznana agentka „{agent}”.")));
        }
        let broker = self
            .broker
            .clone()
            .ok_or_else(|| AppError::unavailable("Podgląd pulpitu", "safety-broker"))?;
        let app = safety_broker_contract::AppSelector::parse(DESKTOP_APP)
            .map_err(|e| AppError::internal(format!("zdolność pulpitu: {e}")))?;
        let holder = Holder {
            session,
            agent: Some(AgentId::new(agent)),
            role: None,
        };
        let mut facts = DeclaredFacts::new("screen_capture");
        facts.touches_private_data = true;
        let request = ActionRequest {
            holder: holder.clone(),
            capability: Capability::GuiControl(app),
            facts,
            origin: CommandOrigin::UserText,
            ttl_ms: None,
        };
        let decision = broker
            .decide(request)
            .await
            .map_err(|e| AppError::new(ErrorCode::Forbidden, format!("Broker: {e}")))?;
        match decision {
            Decision::Allow(token) => {
                // Polityka już pozwala (poziom autonomii) — token z prośby nie będzie użyty.
                if let Err(e) = broker.revoke(token.id).await {
                    tracing::warn!(error = %e, "unieważnienie tokenu podglądu nie powiodło się");
                }
                Ok(BrokerIntentResult {
                    status: BrokerIntentStatus::Applied,
                    request_id: String::new(),
                })
            }
            Decision::NeedsApproval(ticket) => {
                let shown = self.port.open_approval(&ticket.id.0.to_string()).await?;
                spawn_token_watch(broker, ticket.id, holder);
                Ok(shown)
            }
            Decision::Deny(DenyReason::KernelBlock(rule)) => Err(AppError::forbidden(format!(
                "Twarda blokada Jądra: {rule:?} — podgląd pulpitu niedostępny."
            ))),
            Decision::Deny(DenyReason::AuditUnavailable) => Err(AppError::new(
                ErrorCode::Unavailable,
                "Audyt niedostępny — Broker nie wydaje zgód (bezpieczna odmowa).",
            )),
        }
    }
}

/// Po decyzji w oknie Brokera: token jednorazowy z prośby (nieużyty) jest unieważniany —
/// zostaje tylko zakres „zawsze zezwalaj", jeśli właściciel go wybrał.
fn spawn_token_watch(broker: Arc<dyn Broker>, id: ApprovalId, holder: Holder) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    handle.spawn(async move {
        let deadline = tokio::time::Instant::now() + GRANT_WATCH;
        while tokio::time::Instant::now() < deadline {
            match broker.approval_status(id, &holder) {
                Ok(ApprovalStatus::Pending) => tokio::time::sleep(GRANT_POLL).await,
                Ok(ApprovalStatus::Approved { token: Some(token) }) => {
                    if let Err(e) = broker.revoke(token.id).await {
                        tracing::warn!(error = %e, "unieważnienie tokenu podglądu nie powiodło się");
                    }
                    return;
                }
                Ok(_) | Err(_) => return,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_ids_are_validated() {
        assert!(valid_agent("delta"));
        assert!(valid_agent("agentka-2"));
        assert!(!valid_agent(""));
        assert!(!valid_agent("Delta"));
        assert!(!valid_agent("../x"));
    }
}
