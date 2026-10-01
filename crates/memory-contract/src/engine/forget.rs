//! `forget` kaskadowo: nasiona wg celu → plan ([`crate::plan_cascade`]) → usunięcie we wszystkich
//! zakresach (wpis + FTS + wektor), przywrócenie zastąpionych, czyszczenie dziennika i pamięci
//! podręcznej, crypto-shredding baz zakresów, raport eksportów do ponownego wygenerowania.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde_json::json;

use super::MemoryEngine;
use crate::access::{Accessor, Op, authorize, require_owner};
use crate::backend::{MemoryBackend, StoreOp};
use crate::cascade::{CascadePlan, plan_cascade};
use crate::error::MemoryError;
use crate::events as names;
use crate::model::{EntryRef, scope_key, validate_scope};
use crate::service::{CascadeReport, ForgetTarget};
use crate::types::{MemoryEntry, MemoryId, MemoryScope, Provenance};

impl<B: MemoryBackend> MemoryEngine<B> {
    /// Zakresy, w których mogą leżeć nasiona i pochodne celu.
    fn scopes_for(&self, target: &ForgetTarget) -> Result<Vec<MemoryScope>, MemoryError> {
        let mut scopes = match target {
            ForgetTarget::Entry(r) => vec![r.scope.clone()],
            ForgetTarget::Scope(s) => vec![s.clone()],
            ForgetTarget::Session(s) | ForgetTarget::Turn { session: s, .. } => {
                vec![MemoryScope::Session(s.clone())]
            }
            ForgetTarget::Source(_) => self.backend.scopes()?,
        };
        for s in self.broader_scopes()? {
            if !scopes.contains(&s) {
                scopes.push(s);
            }
        }
        Ok(scopes)
    }

    fn authorize_forget(&self, who: &Accessor, target: &ForgetTarget) -> Result<(), MemoryError> {
        match (who, target) {
            (Accessor::Owner, _) => Ok(()),
            (Accessor::Agent(_), ForgetTarget::Entry(r)) => authorize(who, &r.scope, Op::Write),
            _ => require_owner(who, "zapomnienie zakresu, sesji lub źródła"),
        }
    }

    pub(super) fn do_forget(
        &self,
        who: &Accessor,
        target: &ForgetTarget,
    ) -> Result<CascadeReport, MemoryError> {
        self.authorize_forget(who, target)?;
        let scope_check = match target {
            ForgetTarget::Entry(r) => Some(&r.scope),
            ForgetTarget::Scope(s) => Some(s),
            _ => None,
        };
        if let Some(s) = scope_check {
            validate_scope(s)?;
        }
        let scopes = self.scopes_for(target)?;
        let mut all: Vec<MemoryEntry> = Vec::new();
        for s in &scopes {
            all.extend(self.backend.entries(s)?);
        }
        let seeds: BTreeSet<EntryRef> = match target {
            ForgetTarget::Entry(r) => {
                if !all.iter().any(|e| e.scope == r.scope && e.id == r.id) {
                    return Err(MemoryError::NotFound { id: r.id.clone() });
                }
                BTreeSet::from([r.clone()])
            }
            ForgetTarget::Scope(s) => refs(all.iter().filter(|e| &e.scope == s)),
            ForgetTarget::Session(s) => refs(all.iter().filter(|e| {
                e.scope == MemoryScope::Session(s.clone()) || e.origin.session.as_ref() == Some(s)
            })),
            ForgetTarget::Turn { session, turn } => refs(all.iter().filter(|e| {
                e.origin.session.as_ref() == Some(session) && e.origin.turn == Some(*turn)
            })),
            ForgetTarget::Source(src) => refs(all.iter().filter(|e| match &e.provenance {
                Provenance::UntrustedContent { source } | Provenance::Import { source } => {
                    source == src
                }
                _ => false,
            })),
        };
        let family = matches!(target, ForgetTarget::Entry(_));
        let plan = plan_cascade(&all, &seeds, family);
        if matches!(who, Accessor::Agent(_)) {
            for r in plan.remove.iter().chain(&plan.revive) {
                authorize(who, &r.scope, Op::Write).map_err(|_| {
                    MemoryError::forbidden(
                        "zapomnienie objęłoby wpisy poza zakresami agentki — decyzja użytkownika",
                    )
                })?;
            }
        }
        let drop = match target {
            ForgetTarget::Scope(s) => Some(s.clone()),
            ForgetTarget::Session(s) => Some(MemoryScope::Session(s.clone())),
            _ => None,
        };
        let report = self.execute_plan(&plan, &all, &scopes, drop.as_ref())?;
        self.emit(
            names::FORGOTTEN,
            None,
            json!({
                "removed": report.removed.len(), "derived": report.derived.len(),
                "versions": report.versions.len(), "revived": report.revived.len(),
                "fts_rows": report.fts_rows, "vectors": report.vectors,
                "journal": report.journal_records,
                "shredded": report.shredded.iter().map(scope_key).collect::<Vec<_>>(),
            }),
        );
        Ok(report)
    }

    /// Wykonuje plan: najpierw zakresy szersze (pochodne), potem węższe; `drop` — zakres usuwany
    /// w całości (baza własna → crypto-shredding).
    pub(super) fn execute_plan(
        &self,
        plan: &CascadePlan,
        all: &[MemoryEntry],
        scopes: &[MemoryScope],
        drop: Option<&MemoryScope>,
    ) -> Result<CascadeReport, MemoryError> {
        let mut report = CascadeReport {
            cache_entries: self.cache.clear(),
            removed: plan.remove.iter().cloned().collect(),
            derived: plan.derived.iter().cloned().collect(),
            versions: plan.versions.iter().cloned().collect(),
            revived: plan.revive.iter().cloned().collect(),
            ..CascadeReport::default()
        };
        let removed_ids: BTreeSet<&MemoryId> = plan.remove.iter().map(|r| &r.id).collect();
        let oldest: BTreeMap<&MemoryScope, DateTime<Utc>> =
            plan.remove.iter().fold(BTreeMap::new(), |mut m, r| {
                if let Some(e) = all.iter().find(|e| e.scope == r.scope && e.id == r.id) {
                    let t = m.entry(&r.scope).or_insert(e.created_at);
                    *t = (*t).min(e.created_at);
                }
                m
            });
        let mut ordered: Vec<&MemoryScope> = scopes.iter().collect();
        ordered.sort_by_key(|s| match s {
            MemoryScope::Global => 0,
            MemoryScope::Project(_) | MemoryScope::Agent(_) => 1,
            MemoryScope::Session(_) => 2,
        });
        for scope in ordered {
            let exports = self.backend.exports(scope)?;
            if Some(scope) == drop {
                report
                    .stale_exports
                    .extend(exports.into_iter().map(|n| n.name));
                let dropped = self.backend.drop_scope(scope)?;
                report.fts_rows += dropped.commit.fts_rows;
                report.vectors += dropped.commit.vectors;
                report.journal_records += dropped.commit.journal_deleted;
                if dropped.shredded {
                    report.shredded.push(scope.clone());
                }
                continue;
            }
            let mut ops: Vec<StoreOp> = plan
                .remove
                .iter()
                .filter(|r| &r.scope == scope)
                .map(|r| StoreOp::Delete(r.id.clone()))
                .collect();
            let removing = !ops.is_empty();
            for r in plan.revive.iter().filter(|r| &r.scope == scope) {
                if let Some(mut e) = all
                    .iter()
                    .find(|e| e.scope == r.scope && e.id == r.id)
                    .cloned()
                {
                    e.superseded = None;
                    ops.push(StoreOp::Put(Box::new(e)));
                }
            }
            for record in self.backend.journal(scope)? {
                if record.refs.iter().any(|id| removed_ids.contains(id)) {
                    ops.push(StoreOp::DeleteJournal(record.id));
                }
            }
            if removing && let Some(since) = oldest.get(scope) {
                report.stale_exports.extend(
                    exports
                        .into_iter()
                        .filter(|n| n.at >= *since)
                        .map(|n| n.name),
                );
            }
            let commit = self.commit(scope, ops)?;
            report.fts_rows += commit.fts_rows;
            report.vectors += commit.vectors;
            report.journal_records += commit.journal_deleted;
        }
        report.stale_exports.sort();
        report.stale_exports.dedup();
        Ok(report)
    }
}

fn refs<'a>(entries: impl Iterator<Item = &'a MemoryEntry>) -> BTreeSet<EntryRef> {
    entries.map(MemoryEntry::entry_ref).collect()
}
