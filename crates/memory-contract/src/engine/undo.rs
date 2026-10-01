//! Cofanie zmiany z dziennika (właściciel). Utworzone wpisy znikają kaskadowo (zastąpione wracają),
//! scalenia i oznaczenia wracają do migawki „przed”; wygaszenie jest nieodwracalne.

use std::collections::BTreeSet;

use serde_json::json;

use super::MemoryEngine;
use crate::access::{Accessor, require_owner};
use crate::backend::{MemoryBackend, StoreOp};
use crate::cascade::plan_cascade;
use crate::error::MemoryError;
use crate::events as names;
use crate::journal::{ChangeId, ChangeKind, UndoReport};
use crate::model::{EntryRef, scope_key, validate_scope};
use crate::types::{MemoryEntry, MemoryId, MemoryScope};

impl<B: MemoryBackend> MemoryEngine<B> {
    pub(super) fn do_undo(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
        change: &ChangeId,
    ) -> Result<UndoReport, MemoryError> {
        require_owner(who, "cofnięcie zmiany pamięci")?;
        validate_scope(scope)?;
        let record = self
            .backend
            .journal(scope)?
            .into_iter()
            .find(|r| &r.id == change)
            .ok_or_else(|| MemoryError::invalid(format!("brak zmiany {change} w dzienniku")))?;
        if record.undone {
            return Err(MemoryError::conflict("zmiana już cofnięta"));
        }
        let mut report = UndoReport {
            change: Some(change.clone()),
            ..UndoReport::default()
        };
        match record.kind {
            ChangeKind::Expire => {
                return Err(MemoryError::conflict(
                    "wygaszenie (retencja) jest nieodwracalne",
                ));
            }
            ChangeKind::Create | ChangeKind::Promote | ChangeKind::Supersede | ChangeKind::Edit => {
                let before: BTreeSet<&MemoryId> = record.before.iter().map(|e| &e.id).collect();
                let created: BTreeSet<EntryRef> = record
                    .after
                    .iter()
                    .filter(|e| !before.contains(&e.id))
                    .map(MemoryEntry::entry_ref)
                    .collect();
                let mut scopes = vec![scope.clone()];
                scopes.extend(self.broader_scopes()?.into_iter().filter(|s| s != scope));
                let mut all = Vec::new();
                for s in &scopes {
                    all.extend(self.backend.entries(s)?);
                }
                let (present, gone): (BTreeSet<EntryRef>, BTreeSet<EntryRef>) = created
                    .into_iter()
                    .partition(|r| all.iter().any(|e| e.scope == r.scope && e.id == r.id));
                report.skipped.extend(gone);
                let plan = plan_cascade(&all, &present, false);
                let cascade = self.execute_plan(&plan, &all, &scopes, None)?;
                report.removed = cascade.removed;
                report.restored = cascade.revived;
                let mut ops = Vec::new();
                for snapshot in &record.before {
                    let after = record.after.iter().find(|a| a.id == snapshot.id);
                    if let (Some(mut current), Some(after)) =
                        (self.backend.entry(scope, &snapshot.id)?, after)
                        && current.superseded == after.superseded
                        && current.superseded != snapshot.superseded
                    {
                        current.superseded = snapshot.superseded.clone();
                        report.restored.push(current.entry_ref());
                        ops.push(StoreOp::Put(Box::new(current)));
                    }
                }
                if let Some(mut done) = self
                    .backend
                    .journal(scope)?
                    .into_iter()
                    .find(|r| &r.id == change)
                {
                    done.undone = true;
                    ops.push(StoreOp::PutJournal(Box::new(done)));
                }
                self.commit(scope, ops)?;
            }
            ChangeKind::Merge | ChangeKind::MarkConsolidated => {
                let mut ops = Vec::new();
                let keep = record.refs.first();
                for snapshot in &record.before {
                    let current = self.backend.entry(scope, &snapshot.id)?;
                    let restored = match (record.kind, current) {
                        (ChangeKind::Merge, Some(mut cur))
                            if cur.superseded.as_ref().map(|s| &s.by) == keep =>
                        {
                            cur.superseded = snapshot.superseded.clone();
                            Some(cur)
                        }
                        (ChangeKind::MarkConsolidated, Some(mut cur)) => {
                            cur.consolidated_at = snapshot.consolidated_at;
                            Some(cur)
                        }
                        _ => None,
                    };
                    match restored {
                        Some(e) => {
                            report.restored.push(e.entry_ref());
                            ops.push(StoreOp::Put(Box::new(e)));
                        }
                        None => report.skipped.push(snapshot.entry_ref()),
                    }
                }
                let mut done = record.clone();
                done.undone = true;
                ops.push(StoreOp::PutJournal(Box::new(done)));
                self.commit(scope, ops)?;
            }
            ChangeKind::Conflict => {
                let mut done = record.clone();
                done.undone = true;
                self.commit(scope, vec![StoreOp::PutJournal(Box::new(done))])?;
            }
        }
        self.emit(
            names::CHANGE_UNDONE,
            None,
            json!({ "scope": scope_key(scope), "change": change, "removed": report.removed.len(),
                    "restored": report.restored.len() }),
        );
        Ok(report)
    }
}
