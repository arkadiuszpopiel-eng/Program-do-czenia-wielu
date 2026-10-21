//! Komendy `permissions_*`: poziomy autonomii z Brokera; zmiany i zatwierdzenia wyłącznie przez
//! Brokera (⟶ `BrokerPort`) — obniżenie od razu, podniesienie tylko w oknie Brokera (PLAN §8.2).

use crate::core::AppCore;
use crate::dto::{
    AutonomyLevel, BrokerIntentResult, BrokerIntentStatus, BrokerStatusView, PermissionsState,
};
use std::collections::BTreeMap;

use app_broker::levels;
use sessions_contract::SessionId;

use crate::error::AppError;
use crate::ids;
use crate::ports::ApprovalOutcome;
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
            // Broker potwierdzi zgodę na tę prośbę, zamiast cofać do zapamiętanego obniżenia.
            let scope = session.as_ref().map(SessionId::as_str);
            self.inner.raises.add(scope, level, &result.request_id);
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
        (
            global,
            levels::parse_session_levels(self.config_list(SESSION_LEVELS).await),
        )
    }

    /// Lista napisów z konfiguracji (brak albo inny typ — pusta).
    async fn config_list(&self, key: &str) -> Vec<String> {
        self.config_value(key)
            .await
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }

    /// Zapisuje obniżenie poziomu sesji (wraca po restarcie aplikacji i Brokera).
    async fn remember_session_level(
        &self,
        session: &SessionId,
        level: AutonomyLevel,
    ) -> Result<(), AppError> {
        let (_, mut sessions) = self.remembered_levels().await;
        sessions.insert(session.as_str().to_owned(), level);
        let entries = levels::format_session_levels(&sessions);
        let value = serde_json::to_value(entries).map_err(AppError::internal)?;
        self.config_set(SESSION_LEVELS, Some(value), false).await
    }

    /// Nakłada zapamiętane obniżenia na Brokera, gdy ma on poziom wyższy (np. po jego restarcie
    /// albo opóźnionym starcie — audyt 2026-10-14, A-04). `false` — Broker nieosiągalny, stan
    /// nieznany (próba zostanie powtórzona). Podniesień nie przywraca: te wymagają zgody w oknie
    /// Brokera — zatwierdzone tam najpierw przyjmuje jako nowe zapamiętane poziomy.
    pub(crate) async fn enforce_autonomy(&self) -> bool {
        let Some(view) = self.inner.broker.levels(None).await else {
            return false;
        };
        // Poziomy sesji odczytane przed sprawdzeniem zgód, jak `view.global`: zgoda udzielona
        // w trakcie przebiegu nie jest w nim cofana — przyjmie ją następny (przegląd 2026-10-21).
        let mut before = Vec::new();
        for id in self.remembered_levels().await.1.into_keys() {
            let Ok(session) = ids::session(&id) else {
                continue;
            };
            if self.ensure_session(&session).is_err() {
                continue;
            }
            if let Some(current) = self.session_level(&session).await {
                before.push((id, session, current));
            }
        }
        let held = self.adopt_approved_raises().await;
        let (global, sessions) = self.remembered_levels().await;
        // Zapamiętane L4, a Broker na domyślnym L3 (restart, wgranie wersji): poziom wracał do L3
        // bez słowa. Informacja dopiero przy trzecim takim przebiegu z rzędu (≥ 30 s: okno słucha
        // zdarzeń, stan nie jest przejściowy jak łączenie z Brokerem) — raz, do zmiany stanu.
        let waiting = global == Some(AutonomyLevel::L4)
            && view.global == AutonomyLevel::L3
            && held.is_empty();
        let passes = {
            let mut rt = self.rt();
            rt.level_notice = if waiting {
                rt.level_notice.saturating_add(1)
            } else {
                0
            };
            rt.level_notice
        };
        if passes == 3 {
            self.emit(crate::dto::AlfaEvent::Toast {
                kind: crate::dto::ToastKind::Warning,
                message: crate::dto::LocalizedText::new(
                    "Po uruchomieniu obowiązuje L3 — zapamiętany L4 wymaga ponownego potwierdzenia: Ustawienia → Uprawnienia.",
                    "After the restart L3 applies — the remembered L4 needs confirming again: Settings → Permissions.",
                ),
            });
        }
        if let Some(level) = global
            && level < view.global
            && !held.contains(&None)
        {
            match self.inner.broker.request_level(level, None).await {
                Ok(_) => tracing::info!(?level, "przywrócono zapamiętany poziom autonomii"),
                Err(e) => {
                    tracing::warn!(error = %e, "przywrócenie poziomu autonomii nie powiodło się")
                }
            }
        }
        for (id, session, current) in before {
            if let Some(level) = sessions.get(&id).copied()
                && level < current
                && !held.contains(&Some(id.clone()))
                && let Err(e) = self.inner.broker.request_level(level, Some(&session)).await
            {
                tracing::warn!(error = %e, sesja = %id, "przywrócenie poziomu sesji nie powiodło się");
            }
        }
        true
    }

    /// Poziom obowiązujący sesję w Brokerze (`None` — Broker nieosiągalny).
    async fn session_level(&self, session: &SessionId) -> Option<AutonomyLevel> {
        self.inner
            .broker
            .levels(Some(session))
            .await
            .map(|v| v.session.unwrap_or(v.global))
    }

    /// Podniesienia zatwierdzone w oknie Brokera stają się zapamiętanymi poziomami i są ogłaszane
    /// (zgłoszenia 20.10 i 21.10: L4 po zgodzie wracało do L2, strona pokazywała stary poziom).
    /// Zgodę potwierdza Broker dla tej prośby — sama zgodność poziomu nie wystarcza, bo nowy Broker
    /// startuje z L3 (przegląd 2026-10-21). Odrzucone, wygasłe i nieznane Brokerowi — usuwane.
    /// Zwraca zakresy zatwierdzone, ale niezapisane (błąd zapisu): tych strażnik teraz nie obniża.
    async fn adopt_approved_raises(&self) -> Vec<Option<String>> {
        let mut held = Vec::new();
        for raise in self.inner.raises.current() {
            match self.inner.broker.approval_outcome(&raise.approval).await {
                Some(ApprovalOutcome::Approved) => {}
                Some(ApprovalOutcome::Rejected) => {
                    self.inner.raises.remove(&raise.approval);
                    continue;
                }
                Some(ApprovalOutcome::Pending) | None => continue,
            }
            let session = match raise.session.as_deref().map(ids::session) {
                None => None,
                Some(Ok(s)) if self.ensure_session(&s).is_ok() => Some(s),
                Some(_) => {
                    self.inner.raises.remove(&raise.approval);
                    continue;
                }
            };
            // Poziom czytany po stanie prośby: Broker zmienia oba pod jedną blokadą.
            let current = match &session {
                None => self.inner.broker.levels(None).await.map(|v| v.global),
                Some(s) => self.session_level(s).await,
            };
            let Some(current) = current else {
                continue;
            };
            // Zgoda na starszą prośbę, a Broker ma już inny poziom (późniejsza zmiana) — zostaje
            // zapamiętany poziom.
            if current != raise.level || self.adopt_raise(session.as_ref(), raise.level).await {
                self.inner.raises.remove(&raise.approval);
            } else {
                held.push(raise.session.clone());
            }
        }
        held
    }

    /// Zatwierdzone podniesienie staje się zapamiętanym poziomem (globalnym albo sesji); `false` —
    /// zapis się nie powiódł (prośba zostaje do następnego przebiegu).
    async fn adopt_raise(&self, session: Option<&SessionId>, level: AutonomyLevel) -> bool {
        let saved = match session {
            None => match serde_json::to_value(level) {
                Ok(value) => self.config_set(keys::AUTONOMY, Some(value), false).await,
                Err(e) => Err(AppError::internal(e)),
            },
            Some(id) => self.remember_session_level(id, level).await,
        };
        if let Err(e) = saved {
            tracing::warn!(error = %e, "zapis zatwierdzonego poziomu autonomii nie powiódł się");
            return false;
        }
        tracing::info!(
            ?level,
            "przyjęto podniesienie poziomu zatwierdzone w oknie Brokera"
        );
        match session {
            Some(id) => self.announce_session(id).await,
            None => self.announce_all_sessions().await,
        }
        true
    }

    /// Strażnik autonomii: zapamiętane obniżenia nakładane przy starcie i stale pilnowane — Broker
    /// startujący później albo uruchomiony ponownie nie zostaje z poziomem domyślnym.
    pub(crate) fn spawn_autonomy_guard(&self) {
        let core = self.clone();
        tokio::spawn(async move {
            // Podniesienia zapisywane w konfiguracji przez 0.0.66–0.0.67 — już nieczytane.
            if core.config_value(OLD_PENDING_RAISES).await.is_some()
                && let Err(e) = core.config_set(OLD_PENDING_RAISES, None, false).await
            {
                tracing::debug!(error = %e, "usunięcie starych podniesień nie powiodło się");
            }
            let mut known = false;
            loop {
                known = core.enforce_autonomy().await || known;
                // Prośba czeka na zgodę — częściej, żeby zgoda w oknie Brokera była szybko przyjęta;
                // nowa prośba budzi strażnika od razu.
                let pause = if known && core.inner.raises.is_empty() {
                    GUARD_TICK
                } else {
                    GUARD_RETRY
                };
                tokio::select! {
                    () = tokio::time::sleep(pause) => {}
                    () = core.inner.raises.added() => {}
                }
            }
        });
    }
}

/// Poziomy sesji obniżone przez właściciela: wpisy `identyfikator-sesji:poziom`.
const SESSION_LEVELS: &str = "permissions.session_levels";
/// Dawny zapis podniesień czekających na zgodę (0.0.66–0.0.67) — usuwany przy starcie.
const OLD_PENDING_RAISES: &str = "permissions.pending_raises";
/// Odstęp sprawdzeń, gdy Broker jeszcze nie odpowiedział (opóźniony start) albo prośba czeka.
const GUARD_RETRY: std::time::Duration = std::time::Duration::from_secs(2);
/// Odstęp sprawdzeń po połączeniu (restart Brokera przywraca poziom domyślny).
const GUARD_TICK: std::time::Duration = std::time::Duration::from_secs(15);
