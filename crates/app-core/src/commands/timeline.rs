//! Komenda `timeline_list`: Oś czasu v0 (wywołania modeli z kosztem i opóźnieniem).

use crate::core::AppCore;
use crate::dto::{TimelineEvent, TimelineFilter};
use crate::error::AppError;
use crate::ids;

impl AppCore {
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
