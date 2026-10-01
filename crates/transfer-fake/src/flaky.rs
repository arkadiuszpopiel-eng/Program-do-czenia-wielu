//! Sesje zawodzące po wyczerpaniu [`FailureBudget`] — każda operacja zapisu zużywa jedną
//! jednostkę. Do testów „import przerwany w połowie” (`ACC-F1-transfer-03`).

use std::sync::Arc;

use sessions_contract::{
    DeleteReport, HeardPrefix, NewSession, NewTurn, PortableSession, SessionCatalog, SessionError,
    SessionHistory, SessionId, SessionMeta, SessionPatch, SessionQuery, SessionSummary, Sessions,
    Siblings, Turn, TurnId,
};

use crate::ports::FailureBudget;

/// Opakowanie sesji z budżetem awarii zapisu.
pub struct FlakySessions {
    inner: Arc<dyn Sessions>,
    budget: FailureBudget,
}

impl FlakySessions {
    /// Opakowuje `inner`; zapisy zużywają `budget`.
    pub fn new(inner: Arc<dyn Sessions>, budget: FailureBudget) -> Self {
        Self { inner, budget }
    }

    fn spend(&self, what: &str) -> Result<(), SessionError> {
        self.budget.spend(what).map_err(SessionError::storage)
    }
}

impl SessionCatalog for FlakySessions {
    fn create_session(&self, new: NewSession) -> Result<SessionMeta, SessionError> {
        self.spend("create_session")?;
        self.inner.create_session(new)
    }
    fn session(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        self.inner.session(id)
    }
    fn update_session(
        &self,
        id: &SessionId,
        patch: SessionPatch,
    ) -> Result<SessionMeta, SessionError> {
        self.spend("update_session")?;
        self.inner.update_session(id, patch)
    }
    fn mark_tainted(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        self.spend("mark_tainted")?;
        self.inner.mark_tainted(id)
    }
    fn list_sessions(&self, query: &SessionQuery) -> Result<Vec<SessionSummary>, SessionError> {
        self.inner.list_sessions(query)
    }
    fn set_activity(&self, id: &SessionId, active: bool) -> Result<(), SessionError> {
        self.inner.set_activity(id, active)
    }
    fn mark_read(&self, id: &SessionId) -> Result<(), SessionError> {
        self.inner.mark_read(id)
    }
    fn trash_session(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        self.spend("trash_session")?;
        self.inner.trash_session(id)
    }
    fn restore_session(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        self.spend("restore_session")?;
        self.inner.restore_session(id)
    }
    fn delete_session(&self, id: &SessionId) -> Result<DeleteReport, SessionError> {
        self.spend("delete_session")?;
        self.inner.delete_session(id)
    }
    fn adopt_session(&self, session: PortableSession) -> Result<SessionMeta, SessionError> {
        self.spend("adopt_session")?;
        self.inner.adopt_session(session)
    }
}

impl SessionHistory for FlakySessions {
    fn append_turn(
        &self,
        id: &SessionId,
        parent: Option<TurnId>,
        turn: NewTurn,
    ) -> Result<Turn, SessionError> {
        self.spend("append_turn")?;
        self.inner.append_turn(id, parent, turn)
    }
    fn fork_from(
        &self,
        id: &SessionId,
        sibling_of: TurnId,
        turn: NewTurn,
    ) -> Result<Turn, SessionError> {
        self.spend("fork_from")?;
        self.inner.fork_from(id, sibling_of, turn)
    }
    fn turn(&self, id: &SessionId, turn: TurnId) -> Result<Turn, SessionError> {
        self.inner.turn(id, turn)
    }
    fn branch_projection(&self, id: &SessionId, leaf: TurnId) -> Result<Vec<Turn>, SessionError> {
        self.inner.branch_projection(id, leaf)
    }
    fn siblings(&self, id: &SessionId, turn: TurnId) -> Result<Siblings, SessionError> {
        self.inner.siblings(id, turn)
    }
    fn latest_leaf(&self, id: &SessionId, from: TurnId) -> Result<TurnId, SessionError> {
        self.inner.latest_leaf(id, from)
    }
    fn set_active_leaf(&self, id: &SessionId, leaf: TurnId) -> Result<(), SessionError> {
        self.spend("set_active_leaf")?;
        self.inner.set_active_leaf(id, leaf)
    }
    fn active_leaf(&self, id: &SessionId) -> Result<Option<TurnId>, SessionError> {
        self.inner.active_leaf(id)
    }
    fn record_heard_prefix(
        &self,
        id: &SessionId,
        turn: TurnId,
        prefix: HeardPrefix,
    ) -> Result<Turn, SessionError> {
        self.spend("record_heard_prefix")?;
        self.inner.record_heard_prefix(id, turn, prefix)
    }
    fn set_hidden(&self, id: &SessionId, turn: TurnId, hidden: bool) -> Result<(), SessionError> {
        self.spend("set_hidden")?;
        self.inner.set_hidden(id, turn, hidden)
    }
    fn save_draft(&self, id: &SessionId, text: &str) -> Result<(), SessionError> {
        self.spend("save_draft")?;
        self.inner.save_draft(id, text)
    }
    fn draft(&self, id: &SessionId) -> Result<Option<String>, SessionError> {
        self.inner.draft(id)
    }
    fn turn_count(&self, id: &SessionId) -> Result<u64, SessionError> {
        self.inner.turn_count(id)
    }
    fn all_turns(&self, id: &SessionId) -> Result<Vec<Turn>, SessionError> {
        self.inner.all_turns(id)
    }
    fn import_turns(&self, id: &SessionId, turns: &[Turn]) -> Result<u64, SessionError> {
        self.spend("import_turns")?;
        self.inner.import_turns(id, turns)
    }
}
