//! Historia (drzewo append-only) w atrapie.

use sessions_contract::{
    BranchId, HeardPrefix, NewTurn, Role, SessionError, SessionHistory, SessionId, Siblings, Turn,
    TurnId, validate_heard_prefix, validate_new_turn,
};

use crate::FakeSessions;

impl FakeSessions {
    fn insert_turn(
        &self,
        id: &SessionId,
        parent: Option<TurnId>,
        branch: Option<BranchId>,
        new: NewTurn,
    ) -> Result<Turn, SessionError> {
        let mut st = self.lock();
        st.get(id)?;
        let now = st.now();
        let session = st.get_mut(id)?;
        let branch = branch.unwrap_or_else(|| {
            session.next_branch += 1;
            BranchId(session.next_branch)
        });
        let turn = Turn {
            id: TurnId(session.turns.len() as u64 + 1),
            parent,
            branch,
            role: new.role,
            author: new.author,
            content: new.content,
            usage: new.usage,
            created_at: now,
            heard_prefix: new.heard_prefix,
            hidden: false,
        };
        if turn.role != Role::User {
            session.unread += 1;
        }
        session.last_turn_at = Some(now);
        session.active_leaf = Some(turn.id);
        session.turns.insert(turn.id, turn.clone());
        Ok(turn)
    }
}

impl SessionHistory for FakeSessions {
    fn append_turn(
        &self,
        id: &SessionId,
        parent: Option<TurnId>,
        turn: NewTurn,
    ) -> Result<Turn, SessionError> {
        validate_new_turn(&turn)?;
        let branch = {
            let st = self.lock();
            let session = st.get(id)?;
            match parent {
                None if !session.turns.is_empty() => return Err(SessionError::RootExists),
                None => None,
                Some(p) => {
                    let branch = session.turn(p)?.branch;
                    if !session.children(Some(p)).is_empty() {
                        return Err(SessionError::NotALeaf { turn: p });
                    }
                    Some(branch)
                }
            }
        };
        self.insert_turn(id, parent, branch, turn)
    }

    fn fork_from(
        &self,
        id: &SessionId,
        sibling_of: TurnId,
        turn: NewTurn,
    ) -> Result<Turn, SessionError> {
        validate_new_turn(&turn)?;
        let parent = self.lock().get(id)?.turn(sibling_of)?.parent;
        self.insert_turn(id, parent, None, turn)
    }

    fn turn(&self, id: &SessionId, turn: TurnId) -> Result<Turn, SessionError> {
        Ok(self.lock().get(id)?.turn(turn)?.clone())
    }

    fn branch_projection(&self, id: &SessionId, leaf: TurnId) -> Result<Vec<Turn>, SessionError> {
        let st = self.lock();
        let session = st.get(id)?;
        let mut out = vec![session.turn(leaf)?.clone()];
        while let Some(parent) = out.last().and_then(|t| t.parent) {
            out.push(session.turn(parent)?.clone());
        }
        out.reverse();
        Ok(out)
    }

    fn siblings(&self, id: &SessionId, turn: TurnId) -> Result<Siblings, SessionError> {
        let st = self.lock();
        let session = st.get(id)?;
        let parent = session.turn(turn)?.parent;
        let turns = session.children(parent);
        let index = turns.iter().position(|t| *t == turn).unwrap_or_default();
        Ok(Siblings { turns, index })
    }

    fn latest_leaf(&self, id: &SessionId, from: TurnId) -> Result<TurnId, SessionError> {
        let st = self.lock();
        let session = st.get(id)?;
        let mut current = session.turn(from)?.id;
        while let Some(child) = session.children(Some(current)).last().copied() {
            current = child;
        }
        Ok(current)
    }

    fn set_active_leaf(&self, id: &SessionId, leaf: TurnId) -> Result<(), SessionError> {
        let mut st = self.lock();
        let session = st.get_mut(id)?;
        session.turn(leaf)?;
        session.active_leaf = Some(leaf);
        Ok(())
    }

    fn active_leaf(&self, id: &SessionId) -> Result<Option<TurnId>, SessionError> {
        Ok(self.lock().get(id)?.active_leaf)
    }

    fn record_heard_prefix(
        &self,
        id: &SessionId,
        turn: TurnId,
        prefix: HeardPrefix,
    ) -> Result<Turn, SessionError> {
        let mut st = self.lock();
        let session = st.get_mut(id)?;
        let stored = session
            .turns
            .get_mut(&turn)
            .ok_or(SessionError::TurnNotFound { turn })?;
        if stored.heard_prefix.is_some() {
            return Err(SessionError::HeardPrefixAlreadyRecorded { turn });
        }
        validate_heard_prefix(stored.role, &stored.content, prefix)?;
        stored.heard_prefix = Some(prefix);
        Ok(stored.clone())
    }

    fn set_hidden(&self, id: &SessionId, turn: TurnId, hidden: bool) -> Result<(), SessionError> {
        let mut st = self.lock();
        let session = st.get_mut(id)?;
        let stored = session
            .turns
            .get_mut(&turn)
            .ok_or(SessionError::TurnNotFound { turn })?;
        stored.hidden = hidden;
        Ok(())
    }

    fn save_draft(&self, id: &SessionId, text: &str) -> Result<(), SessionError> {
        let mut st = self.lock();
        st.get_mut(id)?.draft = (!text.is_empty()).then(|| text.to_owned());
        Ok(())
    }

    fn draft(&self, id: &SessionId) -> Result<Option<String>, SessionError> {
        Ok(self.lock().get(id)?.draft.clone())
    }

    fn turn_count(&self, id: &SessionId) -> Result<u64, SessionError> {
        Ok(self.lock().get(id)?.turns.len() as u64)
    }
}
