//! `ChatHost` rdzenia dla silnika czatu (`app-chat`): ogłoszenie sesji, stan łączności po
//! generacji (offline / 429 → `SystemStatusChanged` tylko przy zmianie) i koszty sesji.
//! Rdzeń trzymany słabo — silnik żyje w `Inner`, bez cyklu.

use std::sync::{Arc, Weak};

use app_chat::ChatHost;
use async_trait::async_trait;
use sessions_contract::SessionId;

use crate::core::{AppCore, Inner};
use crate::dto::{AlfaEvent, TurnError, TurnErrorCode};

/// Rdzeń widziany przez silnik czatu.
pub(crate) struct CoreChatHost {
    inner: Weak<Inner>,
}

impl CoreChatHost {
    pub(crate) fn new(core: &AppCore) -> Self {
        Self {
            inner: Arc::downgrade(&core.inner),
        }
    }

    fn core(&self) -> Option<AppCore> {
        self.inner.upgrade().map(|inner| AppCore { inner })
    }
}

#[async_trait]
impl ChatHost for CoreChatHost {
    async fn announce_session(&self, session: &SessionId) {
        if let Some(core) = self.core() {
            core.announce_session(session).await;
        }
    }

    async fn connectivity(&self, error: Option<&TurnError>, answered: bool) {
        if let Some(core) = self.core() {
            core.update_connectivity(error, answered).await;
        }
    }

    async fn costs_changed(&self, session: &SessionId) {
        if let Some(core) = self.core() {
            let costs = core.cost_summary(Some(session)).await;
            core.emit(AlfaEvent::CostsChanged {
                session_id: session.to_string(),
                costs,
            });
        }
    }
}

impl AppCore {
    /// Offline / 429 z wyniku generacji → stan systemu (zdarzenie tylko przy zmianie).
    async fn update_connectivity(&self, error: Option<&TurnError>, answered: bool) {
        let changed = {
            let mut rt = self.rt();
            let before = (rt.online, rt.rate_limit.clone());
            match error.map(|e| e.code) {
                Some(TurnErrorCode::Offline) => rt.online = false,
                Some(TurnErrorCode::RateLimited) => {
                    let at = error
                        .and_then(|e| e.retry_at.as_deref())
                        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                        .map(|t| t.with_timezone(&chrono::Utc));
                    let provider = error.and_then(|e| e.provider.clone()).unwrap_or_default();
                    rt.rate_limit = at.map(|at| (provider, at));
                }
                _ if answered => {
                    rt.online = true;
                    rt.rate_limit = None;
                }
                _ => {}
            }
            before != (rt.online, rt.rate_limit.clone())
        };
        if changed {
            let status = self.status_snapshot().await;
            self.emit(AlfaEvent::SystemStatusChanged { status });
        }
    }
}
