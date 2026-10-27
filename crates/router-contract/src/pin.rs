//! Requested model and account assignment precedence relative to a manual queue.
use crate::Candidate;
/// Explicit request wins; assigned defaults yield to the owner's manual queue.
pub fn select_pin(
    requested: Option<Candidate>,
    assigned: Option<Candidate>,
    manual: bool,
) -> Option<Candidate> {
    requested.or(if manual { None } else { assigned })
}
#[cfg(test)]
mod tests {
    use crate::{Candidate, select_pin};

    #[test]
    fn shared_queue_is_not_replaced_by_agent_assignment() {
        assert_eq!(
            select_pin(None, Some(Candidate::new("openai", "small")), true),
            None
        );
        assert_eq!(
            select_pin(
                Some(Candidate::new("openai", "explicit")),
                Some(Candidate::new("anthropic", "assigned")),
                true
            ),
            Some(Candidate::new("openai", "explicit"))
        );
    }

    #[test]
    fn requested_model_wins_over_agent_account() {
        let arena = Candidate::new("openai", "gpt-6-sol");
        let agent = Candidate::new("anthropic", "claude-opus-5-5");
        assert_eq!(
            select_pin(Some(arena.clone()), Some(agent.clone()), false),
            Some(arena)
        );
        assert_eq!(select_pin(None, Some(agent.clone()), false), Some(agent));
        assert_eq!(select_pin(None, None, false), None);
    }
}
