//! Filtr subskrypcji.

use serde::{Deserialize, Serialize};

use crate::event::{AgentId, Event, EventKind, Level, RunId, SessionId};

/// Filtr subskrypcji: pusta lista rodzajów = wszystkie; `None` = bez ograniczenia.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventFilter {
    /// Dopasowywane rodzaje (dokładne dopasowanie).
    #[serde(default)]
    pub kinds: Vec<EventKind>,
    /// Prefiksy rodzajów własnych, np. `"voice."` dopasuje `voice.dialog.*`.
    #[serde(default)]
    pub kind_prefixes: Vec<String>,
    /// Tylko zdarzenia z tej sesji.
    #[serde(default)]
    pub session: Option<SessionId>,
    /// Tylko zdarzenia tej agentki.
    #[serde(default)]
    pub agent: Option<AgentId>,
    /// Tylko zdarzenia tego przebiegu.
    #[serde(default)]
    pub run: Option<RunId>,
    /// Minimalny poziom (domyślnie `Trace` = wszystko).
    #[serde(default = "EventFilter::default_level")]
    pub min_level: Level,
}

impl EventFilter {
    fn default_level() -> Level {
        Level::Trace
    }

    /// Filtr przepuszczający wszystko.
    pub fn all() -> Self {
        Self {
            min_level: Level::Trace,
            ..Self::default()
        }
    }

    /// Filtr po jednym rodzaju.
    pub fn kind(kind: EventKind) -> Self {
        Self {
            kinds: vec![kind],
            ..Self::all()
        }
    }

    /// Filtr po prefiksie rodzaju własnego.
    pub fn prefix(prefix: impl Into<String>) -> Self {
        Self {
            kind_prefixes: vec![prefix.into()],
            ..Self::all()
        }
    }

    /// Zawęża do sesji (builder).
    #[must_use]
    pub fn with_session(mut self, session: SessionId) -> Self {
        self.session = Some(session);
        self
    }

    /// Zawęża do minimalnego poziomu (builder).
    #[must_use]
    pub fn with_min_level(mut self, level: Level) -> Self {
        self.min_level = level;
        self
    }

    /// Czy zdarzenie spełnia filtr.
    pub fn matches(&self, event: &Event) -> bool {
        if event.level < self.min_level {
            return false;
        }
        let kind_ok = (self.kinds.is_empty() && self.kind_prefixes.is_empty())
            || self.kinds.contains(&event.kind)
            || self
                .kind_prefixes
                .iter()
                .any(|p| event.kind.as_str().starts_with(p.as_str()));
        kind_ok
            && opt_matches(&self.session, &event.session)
            && opt_matches(&self.agent, &event.agent)
            && opt_matches(&self.run, &event.run)
    }
}

fn opt_matches<T: PartialEq>(wanted: &Option<T>, actual: &Option<T>) -> bool {
    match wanted {
        None => true,
        Some(w) => actual.as_ref() == Some(w),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(kind: EventKind, level: Level) -> Event {
        Event::new(kind, level, serde_json::Value::Null)
    }

    #[test]
    fn default_filter_matches_everything() {
        let f = EventFilter::default();
        assert!(f.matches(&ev(EventKind::Tool, Level::Trace)));
        assert!(f.matches(&ev(EventKind::Custom("a.b".into()), Level::Audit)));
    }

    #[test]
    fn kind_and_prefix_and_level() {
        let f = EventFilter::kind(EventKind::Voice).with_min_level(Level::Info);
        assert!(f.matches(&ev(EventKind::Voice, Level::Warn)));
        assert!(!f.matches(&ev(EventKind::Voice, Level::Debug)));
        assert!(!f.matches(&ev(EventKind::Tool, Level::Warn)));

        let p = EventFilter::prefix("voice.");
        assert!(p.matches(&ev(EventKind::Custom("voice.dialog.x".into()), Level::Info)));
        assert!(!p.matches(&ev(EventKind::Custom("gui.x".into()), Level::Info)));
    }

    #[test]
    fn session_narrowing() {
        let f = EventFilter::all().with_session(SessionId::from("s1"));
        assert!(f.matches(&ev(EventKind::Ui, Level::Info).with_session(SessionId::from("s1"))));
        assert!(!f.matches(&ev(EventKind::Ui, Level::Info).with_session(SessionId::from("s2"))));
        assert!(!f.matches(&ev(EventKind::Ui, Level::Info)));
    }
}
