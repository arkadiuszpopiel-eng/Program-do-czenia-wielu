//! Komenda `timeline_list`: Oś czasu v0 (wywołania modeli z kosztem i opóźnieniem, decyzje
//! Routera, cofnięte kroki, decyzje Brokera) i wpis zdarzenia rdzenia.

use sessions_contract::SessionId;

use crate::core::AppCore;
use crate::dto::{self, AlfaEvent, EventLevel, TimelineEvent, TimelineFilter, TimelineKind};
use crate::error::AppError;
use crate::ids;

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
