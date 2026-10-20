//! Komendy `permissions_*`: poziomy autonomii z Brokera; zmiany i zatwierdzenia wyłącznie przez
//! Brokera (⟶ `BrokerPort`) — obniżenie od razu, podniesienie tylko w oknie Brokera (PLAN §8.2).

use crate::core::AppCore;
use crate::dto::{
    AutonomyLevel, BrokerIntentResult, BrokerIntentStatus, BrokerStatusView, PermissionsState,
};
use std::collections::BTreeMap;

use sessions_contract::SessionId;

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
        if result.status == BrokerIntentStatus::OpenedBroker {
            // Podniesienie czeka na zgodę w oknie Brokera: strażnik przyjmie ten poziom, gdy
            // Broker go zgłosi (w ciągu `RAISE_TTL_MS`), zamiast cofać do zapamiętanego obniżenia.
            let mut raises = self.pending_raises().await;
            let scope = session.as_ref().map_or(GLOBAL_SCOPE, SessionId::as_str);
            raises.insert(
                scope.to_owned(),
                (level, chrono::Utc::now().timestamp_millis()),
            );
            self.store_pending_raises(&raises).await?;
        }
        if result.status == BrokerIntentStatus::Applied {
            match &session {
                // Zapamiętany poziom wraca po restarcie (obniżenie nie wymaga zgody).
                None => {
                    let value = serde_json::to_value(level).map_err(AppError::internal)?;
                    self.config_set(keys::AUTONOMY, Some(value), false).await?;
                }
                Some(id) => self.remember_session_level(id, level).await?,
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

    /// Zapamiętane obniżenia: poziom globalny i poziomy sesji (sesje usunięte — pomijane).
    async fn remembered_levels(&self) -> (Option<AutonomyLevel>, BTreeMap<String, AutonomyLevel>) {
        let global = self.config_str(keys::AUTONOMY).await.and_then(|s| {
            serde_json::from_value::<AutonomyLevel>(serde_json::Value::String(s)).ok()
        });
        // Lista wpisów `sesja:poziom` (konfiguracja nie przyjmuje obiektów jako wartości).
        let sessions = self
            .config_value(SESSION_LEVELS)
            .await
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| {
                let (id, level) = entry.rsplit_once(':')?;
                let level = serde_json::from_value(serde_json::Value::String(level.into())).ok()?;
                Some((id.to_owned(), level))
            })
            .collect();
        (global, sessions)
    }

    /// Zapisuje obniżenie poziomu sesji (wraca po restarcie aplikacji i Brokera).
    async fn remember_session_level(
        &self,
        session: &SessionId,
        level: AutonomyLevel,
    ) -> Result<(), AppError> {
        let (_, mut sessions) = self.remembered_levels().await;
        sessions.insert(session.as_str().to_owned(), level);
        let entries: Vec<String> = sessions
            .iter()
            .map(|(id, level)| {
                let level = serde_json::to_value(level)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                format!("{id}:{level}")
            })
            .collect();
        let value = serde_json::to_value(entries).map_err(AppError::internal)?;
        self.config_set(SESSION_LEVELS, Some(value), false).await
    }

    /// Nakłada zapamiętane obniżenia na Brokera, gdy ma on poziom wyższy (np. po jego restarcie
    /// albo opóźnionym starcie — audyt 2026-10-14, A-04). `false` — Broker nieosiągalny, stan
    /// nieznany (próba zostanie powtórzona). Podniesień nie przywraca: te wymagają zgody w oknie
    /// Brokera.
    pub(crate) async fn enforce_autonomy(&self) -> bool {
        let Some(view) = self.inner.broker.levels(None).await else {
            return false;
        };
        let (global, sessions) = self.remembered_levels().await;
        let mut raises = self.pending_raises().await;
        if let Some(level) = global
            && level < view.global
        {
            // Podniesienie zatwierdzone w oknie Brokera — nowy zapamiętany poziom, nie „restart
            // Brokera z poziomem domyślnym” (zgłoszenie 20.10: L4 po zgodzie wracało do L2).
            if raises.remove(GLOBAL_SCOPE).map(|(l, _)| l) == Some(view.global) {
                self.adopt_raise(None, view.global, &raises).await;
                return true;
            }
            match self.inner.broker.request_level(level, None).await {
                Ok(_) => tracing::info!(?level, "przywrócono zapamiętany poziom autonomii"),
                Err(e) => {
                    tracing::warn!(error = %e, "przywrócenie poziomu autonomii nie powiodło się")
                }
            }
        }
        for (id, level) in sessions {
            let Ok(session) = ids::session(&id) else {
                continue;
            };
            if self.ensure_session(&session).is_err() {
                continue;
            }
            let current = self
                .inner
                .broker
                .levels(Some(&session))
                .await
                .map(|v| v.session.unwrap_or(v.global));
            if let Some(c) = current
                && level < c
                && raises.remove(id.as_str()).map(|(l, _)| l) == Some(c)
            {
                self.adopt_raise(Some(&session), c, &raises).await;
                continue;
            }
            if current.is_some_and(|c| level < c)
                && let Err(e) = self.inner.broker.request_level(level, Some(&session)).await
            {
                tracing::warn!(error = %e, sesja = %id, "przywrócenie poziomu sesji nie powiodło się");
            }
        }
        true
    }

    /// Oczekujące podniesienia (zakres → poziom, chwila prośby), bez przeterminowanych.
    async fn pending_raises(&self) -> BTreeMap<String, (AutonomyLevel, i64)> {
        let now = chrono::Utc::now().timestamp_millis();
        self.config_value(PENDING_RAISES)
            .await
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| {
                let mut parts = entry.rsplitn(3, ':');
                let at: i64 = parts.next()?.parse().ok()?;
                let level = parts.next()?;
                let scope = parts.next()?;
                let level = serde_json::from_value(serde_json::Value::String(level.into())).ok()?;
                (now.saturating_sub(at) <= RAISE_TTL_MS).then(|| (scope.to_owned(), (level, at)))
            })
            .collect()
    }

    /// Zapis oczekujących podniesień (wpisy `zakres:poziom:chwila`).
    async fn store_pending_raises(
        &self,
        raises: &BTreeMap<String, (AutonomyLevel, i64)>,
    ) -> Result<(), AppError> {
        let entries: Vec<String> = raises
            .iter()
            .filter_map(|(scope, (level, at))| {
                let level = serde_json::to_value(level).ok()?;
                Some(format!("{scope}:{}:{at}", level.as_str()?))
            })
            .collect();
        let value = serde_json::to_value(entries).map_err(AppError::internal)?;
        self.config_set(PENDING_RAISES, Some(value), false).await
    }

    /// Zatwierdzone podniesienie staje się zapamiętanym poziomem (globalnym albo sesji).
    async fn adopt_raise(
        &self,
        session: Option<&SessionId>,
        level: AutonomyLevel,
        rest: &BTreeMap<String, (AutonomyLevel, i64)>,
    ) {
        let saved = match session {
            None => match serde_json::to_value(level) {
                Ok(value) => self.config_set(keys::AUTONOMY, Some(value), false).await,
                Err(e) => Err(AppError::internal(e)),
            },
            Some(id) => self.remember_session_level(id, level).await,
        };
        if let Err(e) = saved.and(self.store_pending_raises(rest).await) {
            tracing::warn!(error = %e, "zapis zatwierdzonego poziomu autonomii nie powiódł się");
            return;
        }
        tracing::info!(
            ?level,
            "przyjęto podniesienie poziomu zatwierdzone w oknie Brokera"
        );
        match session {
            Some(id) => self.announce_session(id).await,
            None => self.announce_all_sessions().await,
        }
    }

    /// Strażnik autonomii: zapamiętane obniżenia nakładane przy starcie i stale pilnowane — Broker
    /// startujący później albo uruchomiony ponownie nie zostaje z poziomem domyślnym.
    pub(crate) fn spawn_autonomy_guard(&self) {
        let core = self.clone();
        tokio::spawn(async move {
            let mut known = false;
            loop {
                known = core.enforce_autonomy().await || known;
                let pause = if known { GUARD_TICK } else { GUARD_RETRY };
                tokio::time::sleep(pause).await;
            }
        });
    }
}

/// Poziomy sesji obniżone przez właściciela: wpisy `identyfikator-sesji:poziom`.
const SESSION_LEVELS: &str = "permissions.session_levels";
/// Podniesienia czekające na zgodę w oknie Brokera: wpisy `zakres:poziom:chwila` (`*` — globalny).
const PENDING_RAISES: &str = "permissions.pending_raises";
/// Zakres globalny w [`PENDING_RAISES`].
const GLOBAL_SCOPE: &str = "*";
/// Jak długo oczekujące podniesienie jest ważne (karta Brokera wygasa po 10 min).
const RAISE_TTL_MS: i64 = 15 * 60_000;
/// Odstęp sprawdzeń, gdy Broker jeszcze nie odpowiedział (opóźniony start).
const GUARD_RETRY: std::time::Duration = std::time::Duration::from_secs(2);
/// Odstęp sprawdzeń po połączeniu (restart Brokera przywraca poziom domyślny).
const GUARD_TICK: std::time::Duration = std::time::Duration::from_secs(15);
