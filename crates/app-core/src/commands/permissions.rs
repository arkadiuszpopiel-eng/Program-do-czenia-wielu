//! Komendy `permissions_*`: poziomy autonomii z Brokera; zmiany i zatwierdzenia wyłącznie przez
//! Brokera (⟶ `BrokerPort`) — obniżenie od razu, podniesienie tylko w oknie Brokera (PLAN §8.2).

use crate::core::AppCore;
use crate::dto::{
    AutonomyLevel, BrokerIntentResult, BrokerIntentStatus, BrokerStatusView, PermissionsState,
};
use crate::error::AppError;
use crate::ids;
use crate::settings::keys;

impl AppCore {
    /// `permissions_get`.
    pub async fn permissions_get(
        &self,
        session_id: Option<String>,
    ) -> Result<PermissionsState, AppError> {
        let session = match &session_id {
            Some(s) => Some(ids::session(s)?),
            None => None,
        };
        let (global, session) = match self.inner.broker.levels(session.as_ref()).await {
            Some(view) => (view.global, view.session),
            None => (self.autonomy().await, None),
        };
        Ok(PermissionsState {
            global,
            session,
            hello_enabled: self.config_bool("permissions.hello", false).await,
        })
    }

    /// `permissions_request_level` ⟶ Broker: obniżenie od razu (`applied`), podniesienie —
    /// karta w oknie Brokera (bez okna Brokera: odmowa).
    pub async fn permissions_request_level(
        &self,
        level: AutonomyLevel,
        session_id: Option<String>,
    ) -> Result<BrokerIntentResult, AppError> {
        let session = match session_id {
            Some(s) => Some(ids::session(&s)?),
            None => None,
        };
        let result = self
            .inner
            .broker
            .request_level(level, session.as_ref())
            .await?;
        if result.status == BrokerIntentStatus::Applied {
            if session.is_none() {
                // Zapamiętany poziom globalny wraca po restarcie (obniżenie nie wymaga zgody).
                let value = serde_json::to_value(level).map_err(AppError::internal)?;
                self.config_set(keys::AUTONOMY, Some(value), false).await?;
            }
            match &session {
                Some(id) => self.announce_session(id).await,
                None => self.announce_all_sessions().await,
            }
        }
        Ok(result)
    }

    /// `broker_status` — tryb Brokera, łącze, okno zatwierdzeń, watchdog (`app-broker`).
    pub async fn broker_status(&self) -> Result<BrokerStatusView, AppError> {
        Ok(self.inner.broker.status())
    }

    /// `permissions_open_approval` ⟶ karta w oknie Brokera.
    pub async fn permissions_open_approval(
        &self,
        approval_id: String,
    ) -> Result<BrokerIntentResult, AppError> {
        self.inner.broker.open_approval(&approval_id).await
    }

    /// Po starcie: przywraca zapisany niższy poziom globalny w Brokerze (podniesienie po
    /// restarcie wymagałoby ponownej zgody w oknie Brokera — wtedy zostaje poziom domyślny).
    pub(crate) async fn restore_autonomy(&self) {
        let saved = self.config_str(keys::AUTONOMY).await;
        let Some(level) = saved.and_then(|s| {
            serde_json::from_value::<AutonomyLevel>(serde_json::Value::String(s)).ok()
        }) else {
            return;
        };
        let current = self.inner.broker.levels(None).await.map(|v| v.global);
        if current.is_some_and(|c| level < c)
            && let Err(e) = self.inner.broker.request_level(level, None).await
        {
            tracing::warn!(error = %e, "przywrócenie poziomu autonomii nie powiodło się");
        }
    }
}
