//! Komenda `timeline_list`: Oś czasu v0 (wywołania modeli z kosztem i opóźnieniem, decyzje
//! Routera, cofnięte kroki, decyzje Brokera) i wpis zdarzenia rdzenia.

use sessions_contract::SessionId;

use crate::core::AppCore;
use crate::dto::{self, AlfaEvent, EventLevel, TimelineEvent, TimelineFilter, TimelineKind};
use crate::error::AppError;
use crate::ids;
use crate::settings::keys;

impl AppCore {
    /// Zapisuje wpis osi czasu sesji i wysyła `TimelineAppended`.
    pub(crate) fn timeline_note(
        &self,
        session: &SessionId,
        kind: TimelineKind,
        level: EventLevel,
        title: String,
        detail: Option<String>,
    ) {
        let event = TimelineEvent {
            id: ids::timeline_dto(session),
            ts: dto::iso(chrono::Utc::now()),
            session_id: session.to_string(),
            kind,
            level,
            agent: None,
            title,
            detail,
            cost: None,
            latency_ms: None,
            turn_id: None,
        };
        if let Err(e) = self.inner.store.push_timeline(session, &event) {
            tracing::warn!(error = %e, "zapis osi czasu nie powiódł się");
        }
        self.emit(AlfaEvent::TimelineAppended { event });
    }

    /// Wpis „audit” o zmianie bezpieczeństwa w rozmowie (`None` — aktywnej; bez niej — tylko Audyt).
    pub(crate) async fn security_note(
        &self,
        session: Option<&SessionId>,
        level: EventLevel,
        title: String,
    ) {
        let session = match session {
            Some(s) => Some(s.clone()),
            None => self
                .config_str(keys::ACTIVE_SESSION)
                .await
                .and_then(|a| ids::session(&a).ok()),
        };
        if let Some(session) = session
            && self.ensure_session(&session).is_ok()
        {
            self.timeline_note(&session, TimelineKind::Audit, level, title, None);
        }
    }

    /// `timeline_list`.
    pub async fn timeline_list(
        &self,
        session_id: String,
        filter: TimelineFilter,
    ) -> Result<Vec<TimelineEvent>, AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        Ok(self
            .inner
            .store
            .timeline(&id)?
            .into_iter()
            .filter(|e| filter.accepts(e))
            .collect())
    }
}
