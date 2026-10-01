//! Weryfikacja, atenuacja i unieważnianie tokenów.

use safety_broker_contract::{
    AttenuateRequest, BrokerError, CapToken, Capability, EVENT_TOKEN_DENIED, EVENT_TOKEN_ISSUED,
    EVENT_TOKEN_REVOKED, Holder, TokenId,
};
use serde_json::json;

use crate::engine::BrokerEngine;
use crate::state::State;

impl BrokerEngine {
    /// Sprawdza autentyczność i ważność tokenu (bez zakresu potrzebnego użycia).
    fn check_token(
        st: &State,
        t: &CapToken,
        presenter: &Holder,
        now: u64,
    ) -> Result<(), BrokerError> {
        let reject = |why: &str| Err(BrokerError::TokenRejected(why.to_owned()));
        if t.boot != st.keys.boot() {
            return reject("token z innego uruchomienia Brokera");
        }
        if !st.keys.verify(t.key_epoch, &t.signing_bytes(), &t.mac, now) {
            return reject("niezgodny MAC");
        }
        if now >= t.expires_at_ms {
            return Err(BrokerError::TokenExpired);
        }
        if t.issued_at_ms > now {
            return reject("token z przyszłości");
        }
        let Some(meta) = st.tokens.get(&t.id) else {
            return reject("token unieważniony");
        };
        if meta.holder != t.holder
            || meta.parent != t.parent
            || meta.expires_at_ms != t.expires_at_ms
        {
            return reject("token niezgodny z rejestrem");
        }
        if *presenter != t.holder {
            return reject("okaziciel nie jest podmiotem tokenu");
        }
        Ok(())
    }

    /// `Broker::verify`.
    pub(crate) fn verify_sync(
        &self,
        t: &CapToken,
        needed: &Capability,
        presenter: &Holder,
    ) -> Result<(), BrokerError> {
        let now = self.now();
        let mut st = self.lock();
        let result = Self::check_token(&st, t, presenter, now).and_then(|()| {
            if !needed.is_subset_of(&t.cap) {
                return Err(BrokerError::TokenRejected(
                    "użycie poza zakresem tokenu".into(),
                ));
            }
            match st.guard.check_use(needed) {
                Some(rule) => Err(BrokerError::KernelBlock(rule)),
                None => Ok(()),
            }
        });
        if let Err(e) = &result {
            let suspicious = matches!(
                e,
                BrokerError::TokenRejected(_) | BrokerError::KernelBlock(_)
            );
            if suspicious && st.allow_denied_audit(now) {
                let payload = json!({ "token": t.id, "needed": needed, "error": e.to_string() });
                let _ = self.audit(EVENT_TOKEN_DENIED, Some(presenter), payload);
            }
        }
        drop(st);
        result
    }

    /// `Broker::attenuate`.
    pub(crate) fn attenuate_sync(
        &self,
        parent: &CapToken,
        presenter: &Holder,
        req: AttenuateRequest,
    ) -> Result<CapToken, BrokerError> {
        if req.ttl_ms == 0 {
            return Err(BrokerError::InvalidRequest("TTL musi być > 0".into()));
        }
        let now = self.now();
        let mut st = self.lock();
        Self::check_token(&st, parent, presenter, now)?;
        if !req.capability.is_subset_of(&parent.cap) {
            return Err(BrokerError::NotAttenuated);
        }
        if let Some(rule) = st.guard.check_use(&req.capability) {
            return Err(BrokerError::KernelBlock(rule));
        }
        let holder = Holder {
            session: parent.holder.session.clone(),
            agent: parent.holder.agent.clone(),
            role: req.role.or_else(|| parent.holder.role.clone()),
        };
        Self::check_holder(&holder)?;
        let expires = now.saturating_add(req.ttl_ms).min(parent.expires_at_ms);
        let child = Self::mint(
            &mut st,
            holder,
            req.capability,
            Some(parent.id),
            expires,
            now,
        );
        let payload = json!({ "token": child.id, "parent": parent.id, "capability": child.cap, "via": "attenuation" });
        self.audit(EVENT_TOKEN_ISSUED, Some(&child.holder), payload)?;
        Self::register(&mut st, &child);
        Ok(child)
    }

    fn revoke_ids(&self, st: &mut State, roots: &[TokenId], why: &str) -> usize {
        let mut all: Vec<TokenId> = roots.iter().flat_map(|r| st.descendants(*r)).collect();
        all.sort();
        all.dedup();
        for id in &all {
            st.tokens.remove(id);
        }
        if !all.is_empty() {
            let payload = json!({ "tokens": all, "reason": why });
            // Unieważnienie obowiązuje także bez zapisu (bezpieczniej).
            let _ = self.audit(EVENT_TOKEN_REVOKED, None, payload);
        }
        all.len()
    }

    /// `Broker::revoke`.
    pub(crate) fn revoke_sync(&self, id: TokenId) -> Result<usize, BrokerError> {
        let mut st = self.lock();
        if !st.tokens.contains_key(&id) {
            return Err(BrokerError::UnknownToken(id));
        }
        Ok(self.revoke_ids(&mut st, &[id], "revoke"))
    }

    /// `Broker::revoke_holder` (zmiana obsady).
    pub(crate) fn revoke_holder_sync(&self, holder: &Holder) -> usize {
        let mut st = self.lock();
        let roots: Vec<TokenId> = st
            .tokens
            .iter()
            .filter(|(_, m)| m.holder.session == holder.session && m.holder.agent == holder.agent)
            .map(|(id, _)| *id)
            .collect();
        self.revoke_ids(&mut st, &roots, "holder")
    }
}
