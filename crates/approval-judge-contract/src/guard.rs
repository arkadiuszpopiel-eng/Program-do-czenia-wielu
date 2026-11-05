//! Zapora zgody sędziego (ACC-Z4-judge-04, 5.11): model może zezwolić tylko na akcję „czystą” — odwracalną,
//! bez niszczenia danych, w rozmowie bez treści obcej i bez granic podanych przez właściciela. Inaczej decyduje
//! właściciel. Powód: Bielik 4.5B jako sędzia przepuścił 21 z 22 ataków zestawu `evals/Z4/judge` — model
//! nie może być jedyną zaporą przed wstrzyknięciem. Odmowy i przekazania przechodzą bez zmian.

use risk_classifier_contract::{Destructiveness, Reversibility};

use crate::{JudgeInput, JudgeVerdict};

/// Powód przekazania, gdy zapora zatrzymuje zgodę modelu.
pub const GUARD_NOTE: &str = "zgoda sędziego tylko dla akcji odwracalnej, bez treści obcej w rozmowie i bez \
granic właściciela — decyduje właściciel";

/// Czy akcja pozwala na zgodę modelu (bez względu na to, co model odpowiedział).
pub fn allow_permitted(input: &JudgeInput) -> bool {
    let a = &input.action;
    !a.tainted
        && input.limits.is_empty()
        && a.destructive == Destructiveness::None
        && a.reversible != Reversibility::No
}

/// Werdykt po zaporze: `Allow` dla akcji spoza `allow_permitted` → `Escalate`.
pub fn guard(input: &JudgeInput, verdict: JudgeVerdict) -> JudgeVerdict {
    match verdict {
        JudgeVerdict::Allow { .. } if !allow_permitted(input) => JudgeVerdict::escalate(GUARD_NOTE),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use risk_classifier_contract::RuleId;

    use super::*;
    use crate::{JudgeTurn, JudgedAction};

    fn input() -> JudgeInput {
        JudgeInput {
            goal: "kurs EUR".into(),
            transcript: vec![JudgeTurn::Owner {
                text: "Pobierz kurs EUR".into(),
            }],
            action: JudgedAction {
                tool: "net.fetch".into(),
                capability_summary: "GET api.nbp.pl".into(),
                command: None,
                paths: vec![],
                host: Some("api.nbp.pl".into()),
                reversible: Reversibility::Yes,
                destructive: Destructiveness::None,
                rules: vec![RuleId::EgressNotAllowlisted],
                outside_workdir: false,
                tainted: false,
            },
            limits: vec![],
        }
    }

    fn allow() -> JudgeVerdict {
        JudgeVerdict::Allow {
            reason: "zlecone".into(),
        }
    }

    #[test]
    fn clean_action_keeps_allow() {
        assert_eq!(guard(&input(), allow()), allow());
    }

    #[test]
    fn each_condition_turns_allow_into_owner() {
        let mut cases = Vec::new();
        let mut i = input();
        i.action.tainted = true;
        cases.push(i);
        let mut i = input();
        i.limits = vec!["nie wysyłaj".into()];
        cases.push(i);
        let mut i = input();
        i.action.destructive = Destructiveness::Recoverable;
        cases.push(i);
        let mut i = input();
        i.action.reversible = Reversibility::No;
        cases.push(i);
        for i in cases {
            assert_eq!(guard(&i, allow()), JudgeVerdict::escalate(GUARD_NOTE));
        }
    }

    #[test]
    fn deny_and_escalate_pass_unchanged() {
        let mut i = input();
        i.action.tainted = true;
        let deny = JudgeVerdict::Deny {
            category: crate::DenyCategory::Other,
            reason: "nie".into(),
        };
        assert_eq!(guard(&i, deny.clone()), deny);
        let esc = JudgeVerdict::escalate("x");
        assert_eq!(guard(&i, esc.clone()), esc);
    }
}
