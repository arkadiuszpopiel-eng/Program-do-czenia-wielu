//! Komendy kart zgodności mostów CLI (`bridges_*`, Ustawienia → Modele i dostawcy → Mosty) —
//! delegują do `app-bridges::BridgesApp`. Logowanie do CLI wykonuje wyłącznie użytkownik:
//! „Zaloguj w terminalu" otwiera terminal i podaje polecenie do skopiowania.

use crate::core::AppCore;
use crate::dto::{BridgeCard, BridgeLogin};
use crate::error::AppError;

impl AppCore {
    /// `bridges_list`: karty tras CLI/SDK (`refresh` — ponowne wykrycie CLI).
    pub async fn bridges_list(&self, refresh: bool) -> Result<Vec<BridgeCard>, AppError> {
        if refresh {
            self.inner.bridges.detected(true).await;
        }
        Ok(self.inner.bridges.cards().await)
    }

    /// `bridges_set_enabled`: wyłącznik trasy (zabronionej nie da się włączyć).
    pub async fn bridges_set_enabled(
        &self,
        route_id: String,
        enabled: bool,
    ) -> Result<BridgeCard, AppError> {
        self.inner.bridges.set_enabled(&route_id, enabled).await
    }

    /// `bridges_set_schedule`: jawna zgoda na uruchomienia z harmonogramu (0 = brak).
    pub async fn bridges_set_schedule(
        &self,
        bridge: String,
        per_day: u32,
    ) -> Result<BridgeCard, AppError> {
        self.inner.bridges.set_schedule(&bridge, per_day).await
    }

    /// `bridges_pin`: przypięcie wykrytej wersji CLI (`None` — odpięcie).
    pub async fn bridges_pin(
        &self,
        bridge: String,
        version: Option<String>,
    ) -> Result<BridgeCard, AppError> {
        self.inner.bridges.pin(&bridge, version).await
    }

    /// `bridges_open_login`: terminal w katalogu domowym + polecenie logowania do skopiowania.
    pub async fn bridges_open_login(&self, bridge: String) -> Result<BridgeLogin, AppError> {
        let bridges = self.inner.bridges.clone();
        tokio::task::spawn_blocking(move || bridges.open_login(&bridge))
            .await
            .map_err(|e| AppError::internal(format!("terminal: {e}")))?
    }
}
