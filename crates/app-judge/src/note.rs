//! Tekst wpisu dziennika werdyktów (plan 50 §4.3, Z.4) — tytuł i szczegóły wpisu „audit” na osi czasu rozmowy.
//! Powód pochodzi od modelu (może go sterować treść obca), więc jest tylko opisem w szczegółach, nigdy
//! przyciskiem ani poleceniem; jest przycinany.

use approval_judge_contract::{JudgeLogEntry, JudgeVerdict};

/// Najdłuższy powód sędziego w szczegółach wpisu.
const MAX_REASON: usize = 300;

/// Tytuł, szczegóły i ostrzeżenie (`true` — odmowa) wpisu osi czasu.
pub fn describe(entry: &JudgeLogEntry) -> (String, String, bool) {
    let (what, reason, warn) = match &entry.verdict {
        JudgeVerdict::Allow { reason } => ("zezwolił raz", reason.as_str(), false),
        JudgeVerdict::Deny { reason, .. } => ("odmówił", reason.as_str(), true),
        JudgeVerdict::Escalate { reason } => ("przekazał Tobie", reason.as_str(), false),
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
        let why = entry.rejected_because.as_deref().unwrap_or("bez powodu");
        let why: String = why.chars().take(MAX_REASON).collect();
        detail.push_str(&format!(" · Sędzia Cieni nie przyjął werdyktu: {why}"));
    }
    (title, detail, warn)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
            rejected_because: (!accepted).then(|| "termin minął — decyduje właściciel".into()),
        }
    }

    #[test]
    fn deny_is_a_warning_with_category_and_reason() {
        let deny = JudgeVerdict::Deny {
            category: DenyCategory::SecretExfiltration,
            reason: "wysyła klucz".into(),
        };
        let (title, detail, warn) = describe(&entry(deny, true));
        assert!(title.starts_with("Sędzia zgód odmówił ("));
        assert!(title.ends_with("pobranie z x.example.org"));
        assert!(detail.contains("wysyła klucz") && detail.contains("840 ms"));
        assert!(warn);
    }

    #[test]
    fn rejected_allow_says_owner_decided_and_reason_is_clipped() {
        let allow = JudgeVerdict::Allow {
            reason: "x".repeat(1_000),
        };
        let (title, detail, warn) = describe(&entry(allow, false));
        assert!(title.starts_with("Sędzia zgód zezwolił raz"));
        assert!(detail.contains("nie przyjął werdyktu: termin minął"));
        assert!(detail.chars().count() < 600);
        assert!(!warn);
    }
}
