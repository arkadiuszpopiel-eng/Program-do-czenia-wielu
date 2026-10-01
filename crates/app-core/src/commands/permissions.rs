//! Komendy `permissions_*`: stan poziomów autonomii; zmiany i zatwierdzenia wyłącznie w oknie
//! Brokera (⟶ `BrokerPort`) — nigdy w WebView (PLAN §8.2).

use crate::core::AppCore;
use crate::dto::{AutonomyLevel, BrokerIntentResult, PermissionsState};
use crate::error::AppError;
use crate::ids;

impl AppCore {
    /// `permissions_get`.
    pub async fn permissions_get(
        &self,
        session_id: Option<String>,
    ) -> Result<PermissionsState, AppError> {
        if let Some(s) = &session_id {
            ids::session(s)?;
        }
        Ok(PermissionsState {
            global: self.autonomy().await,
            session: None,
            hello_enabled: self.config_bool("permissions.hello", false).await,
        })
    }

    /// `permissions_request_level` ⟶ okno Brokera potwierdza zmianę.
    pub async fn permissions_request_level(
        &self,
        level: AutonomyLevel,
        session_id: Option<String>,
    ) -> Result<BrokerIntentResult, AppError> {
        let session = match session_id {
            Some(s) => Some(ids::session(&s)?),
            None => None,
        };
        self.inner
            .broker
            .request_level(level, session.as_ref())
            .await
    }

    /// `permissions_open_approval` ⟶ karta w oknie Brokera.
    pub async fn permissions_open_approval(
        &self,
        approval_id: String,
    ) -> Result<BrokerIntentResult, AppError> {
        self.inner.broker.open_approval(&approval_id).await
    }
}
