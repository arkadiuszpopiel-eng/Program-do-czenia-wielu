//! Dziennik werdyktów modelu-sędziego (plan 50 §4.3, Z.4): każda ocena trafia na oś czasu rozmowy jako wpis
//! „audit” — widać ją w panelu osi czasu i w dzienniku rodzica. Powód pochodzi od modelu (może go sterować
//! treść obca), więc jest tylko opisem w szczegółach, nigdy przyciskiem ani poleceniem.

use approval_judge_contract::{JudgeLog, JudgeLogEntry, JudgeVerdict};

use crate::core::AppCore;
use crate::dto::{EventLevel, TimelineKind};
use crate::ids;

/// Najdłuższy powód sędziego w szczegółach wpisu.
const MAX_REASON: usize = 300;

/// Zapis werdyktów na osi czasu sesji proszącej agentki.
pub(crate) struct TimelineJudgeLog(pub(crate) AppCore);

/// Tytuł, szczegóły i poziom wpisu osi czasu.
pub(crate) fn describe(entry: &JudgeLogEntry) -> (String, String, EventLevel) {
    let (what, reason, level) = match &entry.verdict {
        JudgeVerdict::Allow { reason } => ("zezwolił raz", reason.as_str(), EventLevel::Info),
        JudgeVerdict::Deny { reason, .. } => ("odmówił", reason.as_str(), EventLevel::Warn),
        JudgeVerdict::Escalate { reason } => ("przekazał Tobie", reason.as_str(), EventLevel::Info),
    };
    let category = match &entry.verdict {
        JudgeVerdict::Deny { category, .. } => format!(" ({})", category.name_pl()),
        _ => String::new(),
    };
    let title = format!("Sędzia zgód {what}{category}: {}", entry.summary);
    let reason: String = reason.chars().take(MAX_REASON).collect();
    let mut detail = format!(
        "{} · powód sędziego: {reason} · model {} · {} ms",
        entry.tool, entry.model, entry.took_ms
    );
    if !entry.accepted {
        detail.push_str(
            " · werdykt nieprzyjęty (termin minął albo zmienił się tryb) — decydował właściciel",
        );
    }
    (title, detail, level)
}

impl JudgeLog for TimelineJudgeLog {
    fn record(&self, entry: JudgeLogEntry) {
        let Ok(session) = ids::session(&entry.session) else {
            return;
        };
        let (title, detail, level) = describe(&entry);
        self.0
            .timeline_note(&session, TimelineKind::Audit, level, title, Some(detail));
    }
}

#[cfg(test)]
mod tests {
    use approval_judge_contract::DenyCategory;

    use super::*;

    fn entry(verdict: JudgeVerdict, accepted: bool) -> JudgeLogEntry {
        JudgeLogEntry {
            session: "s".into(),
            agent: None,
            tool: "net.fetch".into(),
            summary: "pobranie z x.example.org".into(),
            verdict,
            model: "local:bielik".into(),
            took_ms: 840,
            accepted,
        }
    }

    #[test]
    fn deny_is_a_warning_with_category_and_reason() {
        let deny = JudgeVerdict::Deny {
            category: DenyCategory::SecretExfiltration,
            reason: "wysyła klucz".into(),
        };
        let (title, detail, level) = describe(&entry(deny, true));
        assert!(title.starts_with("Sędzia zgód odmówił ("));
        assert!(title.ends_with("pobranie z x.example.org"));
        assert!(detail.contains("wysyła klucz") && detail.contains("840 ms"));
        assert_eq!(level, EventLevel::Warn);
    }

    #[test]
    fn rejected_allow_says_owner_decided_and_reason_is_clipped() {
        let allow = JudgeVerdict::Allow {
            reason: "x".repeat(1_000),
        };
        let (title, detail, level) = describe(&entry(allow, false));
        assert!(title.starts_with("Sędzia zgód zezwolił raz"));
        assert!(detail.contains("decydował właściciel"));
        assert!(detail.chars().count() < 600);
        assert_eq!(level, EventLevel::Info);
    }
}
