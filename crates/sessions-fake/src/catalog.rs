//! Katalog sesji w atrapie.

use std::collections::BTreeMap;

use sessions_contract::{
    DEFAULT_TITLE, DeleteReport, NewSession, PortableSession, SessionCatalog, SessionError,
    SessionId, SessionMeta, SessionPatch, SessionQuery, SessionSummary, apply_query,
    load_or_create_key, normalize_tags, session_key_name,
};

use crate::{FakeSession, FakeSessions};

impl SessionCatalog for FakeSessions {
    fn create_session(&self, new: NewSession) -> Result<SessionMeta, SessionError> {
        let mut st = self.lock();
        st.next_session += 1;
        let id = SessionId::new(format!("sess-{:04}", st.next_session));
        let taken: Vec<String> = st
            .sessions
            .values()
            .map(|s| s.meta.workdir_name())
            .collect();
        let now = st.now();
        let meta = SessionMeta::from_new(new, id.clone(), now, &self.workdir_root, &taken);
        load_or_create_key(self.vault.as_ref(), &session_key_name(&id))?;
        st.sessions.insert(
            id,
            FakeSession {
                meta: meta.clone(),
                turns: BTreeMap::new(),
                next_branch: 0,
                active_leaf: None,
                draft: None,
                unread: 0,
                last_turn_at: None,
            },
        );
        Ok(meta)
    }

    fn session(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        Ok(self.lock().get(id)?.meta.clone())
    }

    fn update_session(
        &self,
        id: &SessionId,
        patch: SessionPatch,
    ) -> Result<SessionMeta, SessionError> {
        let mut st = self.lock();
        st.get(id)?;
        let now = st.now();
        let session = st.get_mut(id)?;
        session.meta.apply(patch, now);
        Ok(session.meta.clone())
    }

    fn mark_tainted(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        let mut st = self.lock();
        let session = st.get_mut(id)?;
        session.meta.tainted = true;
        Ok(session.meta.clone())
    }

    fn list_sessions(&self, query: &SessionQuery) -> Result<Vec<SessionSummary>, SessionError> {
        let st = self.lock();
        let all = st
            .sessions
            .values()
            .map(|s| SessionSummary {
                meta: s.meta.clone(),
                turns: s.turns.len() as u64,
                unread: s.unread,
                active: st.active.contains(&s.meta.id),
                last_turn_at: s.last_turn_at,
            })
            .collect();
        Ok(apply_query(all, query))
    }

    fn set_activity(&self, id: &SessionId, active: bool) -> Result<(), SessionError> {
        let mut st = self.lock();
        st.get(id)?;
        if active {
            st.active.insert(id.clone());
        } else {
            st.active.remove(id);
        }
        Ok(())
    }

    fn mark_read(&self, id: &SessionId) -> Result<(), SessionError> {
        self.lock().get_mut(id)?.unread = 0;
        Ok(())
    }

    fn trash_session(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        set_trashed(self, id, true)
    }

    fn restore_session(&self, id: &SessionId) -> Result<SessionMeta, SessionError> {
        set_trashed(self, id, false)
    }

    fn adopt_session(&self, session: PortableSession) -> Result<SessionMeta, SessionError> {
        session.validate()?;
        let PortableSession {
            mut meta,
            turns,
            active_leaf,
            draft,
        } = session;
        let mut st = self.lock();
        if st.sessions.contains_key(&meta.id) {
            return Err(SessionError::AlreadyExists { id: meta.id });
        }
        if meta.title.trim().is_empty() {
            DEFAULT_TITLE.clone_into(&mut meta.title);
        }
        meta.tags = normalize_tags(&meta.tags);
        load_or_create_key(self.vault.as_ref(), &session_key_name(&meta.id))?;
        let last_turn_at = turns.iter().map(|t| t.created_at).max();
        let next_branch = turns.iter().map(|t| t.branch.0).max().unwrap_or(0);
        st.sessions.insert(
            meta.id.clone(),
            FakeSession {
                meta: meta.clone(),
                turns: turns.into_iter().map(|t| (t.id, t)).collect(),
                next_branch,
                active_leaf,
                draft: draft.filter(|d| !d.is_empty()),
                unread: 0,
                last_turn_at,
            },
        );
        Ok(meta)
    }

    fn delete_session(&self, id: &SessionId) -> Result<DeleteReport, SessionError> {
        let mut st = self.lock();
        st.get(id)?;
        let key_deleted = self.vault.delete(&session_key_name(id))?;
        st.sessions.remove(id);
        st.active.remove(id);
        Ok(DeleteReport {
            key_deleted,
            files_removed: 0,
        })
    }
}

fn set_trashed(
    fake: &FakeSessions,
    id: &SessionId,
    trashed: bool,
) -> Result<SessionMeta, SessionError> {
    let mut st = fake.lock();
    st.get(id)?;
    let now = st.now();
    let session = st.get_mut(id)?;
    session.meta.trashed = trashed;
    session.meta.updated_at = now;
    Ok(session.meta.clone())
}
