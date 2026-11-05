//! Dziennik werdyktów modelu-sędziego (plan 50 §4.3, Z.4): każda ocena trafia na oś czasu rozmowy jako wpis
//! „audit” — widać ją w panelu osi czasu i w dzienniku rodzica. Tekst wpisu: `app_judge::describe`.

use approval_judge_contract::{JudgeLog, JudgeLogEntry};

use crate::core::AppCore;
use crate::dto::{EventLevel, TimelineKind};
use crate::ids;

/// Zapis werdyktów na osi czasu sesji proszącej agentki.
pub(crate) struct TimelineJudgeLog(pub(crate) AppCore);

impl JudgeLog for TimelineJudgeLog {
    fn record(&self, entry: JudgeLogEntry) {
        let Ok(session) = ids::session(&entry.session) else {
            return;
        };
        let (title, detail, warn) = app_judge::describe(&entry);
        let level = if warn {
            EventLevel::Warn
        } else {
            EventLevel::Info
        };
        self.0
            .timeline_note(&session, TimelineKind::Audit, level, title, Some(detail));
    }
}
