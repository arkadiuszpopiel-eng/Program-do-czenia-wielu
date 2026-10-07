//! Zmiany konsolidacji (atomowo w zakresie, z dziennikiem) i cofanie zmian.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use super::write::trust_rank;
use super::{MemoryEngine, validate_f7};
use crate::access::{Accessor, require_owner_or_guardian};
use crate::backend::{MemoryBackend, StoreOp};
use crate::error::MemoryError;
use crate::events as names;
use crate::journal::{ChangeKind, ChangeOp, ChangeReport, ChangeSet, JournalRecord};
use crate::model::{EntryRef, SupersedeReason, Supersession, scope_key, validate_scope};
use crate::types::{MemoryEntry, MemoryId, MemoryScope, NewMemory, Provenance};

/// Stan roboczy zestawu zmian: wpisy zakresu (po zmianach), operacje, raport.
struct Batch {
    entries: BTreeMap<MemoryId, MemoryEntry>,
    ops: Vec<StoreOp>,
    report: ChangeReport,
}

impl Batch {
    fn get(&self, id: &MemoryId) -> Result<MemoryEntry, MemoryError> {
        self.entries
            .get(id)
            .cloned()
            .ok_or_else(|| MemoryError::NotFound { id: id.clone() })
    }

    fn put(&mut self, entry: MemoryEntry) {
        self.ops.push(StoreOp::Put(Box::new(entry.clone())));
        self.entries.insert(entry.id.clone(), entry);
    }
}

impl<B: MemoryBackend> MemoryEngine<B> {
    /// Nowy wpis pochodny z zestawu zmian: źródła w tym samym zakresie, treść niezaufana źródła
    /// przechodzi na pochodną, sesja źródeł dziedziczona, zatwierdzenie w zakresie szerszym tylko
    /// przez właściciela.
    fn derived_entry(
        &self,
        who: &Accessor,
        set: &ChangeSet,
        batch: &Batch,
        mut new: NewMemory,
        approved: bool,
    ) -> Result<MemoryEntry, MemoryError> {
        if new.scope != set.scope {
            return Err(MemoryError::invalid("zmiana poza zakresem zestawu"));
        }
        validate_f7(&new)?;
        let mut parents = Vec::new();
        for r in &new.origin.derived_from {
            if r.scope != set.scope {
                return Err(MemoryError::invalid(
                    "konsolidacja wyprowadza wpisy wyłącznie z tego samego zakresu",
                ));
            }
            parents.push(batch.get(&r.id)?);
        }
        if let Some(bad) = parents.iter().find(|p| !p.trusted) {
            let source = match &bad.provenance {
                Provenance::UntrustedContent { source } => source.clone(),
                _ => "źródło niezaufane".to_owned(),
            };
            new.provenance = Provenance::UntrustedContent { source };
        }
        if new.origin.session.is_none() {
            let sessions: BTreeSet<_> = parents
                .iter()
                .filter_map(|p| p.origin.session.clone())
                .collect();
            if sessions.len() == 1 {
                new.origin.session = sessions.into_iter().next();
            } else if let MemoryScope::Session(s) = &set.scope {
                new.origin.session = Some(s.clone());
            }
        }
        if !matches!(set.scope, MemoryScope::Session(_))
            && let Some(s) = parents
                .iter()
                .filter_map(|p| p.origin.session.as_ref())
                .find(|s| self.ports.privacy.is_private(s))
        {
            return Err(MemoryError::PrivateSource {
                session: s.to_string(),
            });
        }
        self.check_flow(&new)?;
        let broader = !matches!(set.scope, MemoryScope::Session(_));
        let approved = approved && (who.is_owner() || !broader);
        Ok(MemoryEntry::from_new(
            MemoryId(self.new_id("mem")),
            new,
            self.now(),
            approved,
        ))
    }

    fn apply_op(
        &self,
        who: &Accessor,
        set: &ChangeSet,
        batch: &mut Batch,
        op: &ChangeOp,
    ) -> Result<(), MemoryError> {
        let now = self.now();
        let run = Some(set.run.clone());
        let scope = &set.scope;
        let record = match op {
            ChangeOp::Create {
                entry,
                approved,
                note,
            } => {
                let e = self.derived_entry(who, set, batch, entry.clone(), *approved)?;
                let mut refs = vec![e.id.clone()];
                refs.extend(e.origin.derived_from.iter().map(|r| r.id.clone()));
                batch.report.created.push(e.entry_ref());
                batch.put(e.clone());
                JournalRecord {
                    refs,
                    before: vec![],
                    after: vec![e],
                    run,
                    ..self.journal_record(scope, ChangeKind::Create, note)
                }
            }
            ChangeOp::Supersede {
                old,
                entry,
                reason,
                note,
            } => {
                let prev = batch.get(old)?;
                if prev.superseded.is_some() {
                    return Err(MemoryError::conflict("wpis już zastąpiony"));
                }
                let mut e = self.derived_entry(who, set, batch, entry.clone(), prev.approved)?;
                if trust_rank(&e.provenance) < trust_rank(&prev.provenance) {
                    return Err(MemoryError::conflict(
                        "nowsza wersja o niższym zaufaniu — zgłoś konflikt",
                    ));
                }
                e.version = prev.version + 1;
                e.supersedes = Some(prev.id.clone());
                let mut marked = prev.clone();
                marked.superseded = Some(Supersession {
                    by: e.id.clone(),
                    reason: *reason,
                    at: now,
                });
                batch.report.created.push(e.entry_ref());
                batch.report.superseded.push(prev.entry_ref());
                batch.put(marked.clone());
                batch.put(e.clone());
                let refs = vec![prev.id.clone(), e.id.clone()];
                JournalRecord {
                    refs,
                    before: vec![prev],
                    after: vec![marked, e],
                    run,
                    ..self.journal_record(scope, ChangeKind::Supersede, note)
                }
            }
            ChangeOp::Resolve { old, by, note } => {
                let prev = batch.get(old)?;
                let newer = batch.get(by)?;
                if prev.superseded.is_some() || newer.superseded.is_some() || old == by {
                    return Err(MemoryError::conflict("wpis już zastąpiony"));
                }
                if !newer.trusted || trust_rank(&newer.provenance) < trust_rank(&prev.provenance) {
                    return Err(MemoryError::conflict(
                        "nowszy fakt o niższym zaufaniu — zgłoś konflikt",
                    ));
                }
                let mut marked = prev.clone();
                marked.superseded = Some(Supersession {
                    by: by.clone(),
                    reason: SupersedeReason::Contradiction,
                    at: now,
                });
                batch.report.superseded.push(prev.entry_ref());
                batch.put(marked.clone());
                JournalRecord {
                    refs: vec![old.clone(), by.clone()],
                    before: vec![prev],
                    after: vec![marked],
                    run,
                    ..self.journal_record(scope, ChangeKind::Supersede, note)
                }
            }
            ChangeOp::Merge {
                keep,
                duplicates,
                note,
            } => {
                let leader = batch.get(keep)?;
                if leader.superseded.is_some() {
                    return Err(MemoryError::conflict("wpis wiodący już zastąpiony"));
                }
                let (mut before, mut after) = (Vec::new(), Vec::new());
                for id in duplicates.iter().filter(|id| *id != keep) {
                    let dup = batch.get(id)?;
                    if dup.superseded.is_some() {
                        continue;
                    }
                    if dup.trusted && !leader.trusted {
                        return Err(MemoryError::conflict(
                            "zaufany duplikat nie może zostać scalony z niezaufanym",
                        ));
                    }
                    let mut marked = dup.clone();
                    marked.superseded = Some(Supersession {
                        by: keep.clone(),
                        reason: SupersedeReason::Duplicate,
                        at: now,
                    });
                    batch.report.superseded.push(dup.entry_ref());
                    batch.put(marked.clone());
                    before.push(dup);
                    after.push(marked);
                }
                let mut refs = vec![keep.clone()];
                refs.extend(before.iter().map(|e| e.id.clone()));
                JournalRecord {
                    refs,
                    before,
                    after,
                    run,
                    ..self.journal_record(scope, ChangeKind::Merge, note)
                }
            }
            ChangeOp::Expire { id, note } => {
                batch.get(id)?;
                batch.entries.remove(id);
                batch.ops.push(StoreOp::Delete(id.clone()));
                let revive: Vec<MemoryEntry> = batch
                    .entries
                    .values()
                    .filter(|e| e.superseded.as_ref().is_some_and(|s| &s.by == id))
                    .cloned()
                    .collect();
                for mut e in revive {
                    e.superseded = None;
                    batch.put(e);
                }
                batch
                    .report
                    .expired
                    .push(EntryRef::new(scope.clone(), id.clone()));
                JournalRecord {
                    refs: vec![id.clone()],
                    before: vec![],
                    after: vec![],
                    run,
                    ..self.journal_record(scope, ChangeKind::Expire, note)
                }
            }
            ChangeOp::MarkConsolidated { ids } => {
                let mut before = Vec::new();
                let mut after = Vec::new();
                for id in ids {
                    let e = batch.get(id)?;
                    let mut marked = e.clone();
                    marked.consolidated_at = Some(now);
                    batch.put(marked.clone());
                    before.push(e);
                    after.push(marked);
                }
                JournalRecord {
                    refs: ids.clone(),
                    before,
                    after,
                    run,
                    ..self.journal_record(
                        scope,
                        ChangeKind::MarkConsolidated,
                        format!("przetworzono {} epizodów", ids.len()),
                    )
                }
            }
            ChangeOp::FlagConflict { a, b, note } => {
                batch.get(a)?;
                batch.get(b)?;
                batch.report.conflicts += 1;
                JournalRecord {
                    refs: vec![a.clone(), b.clone()],
                    before: vec![],
                    after: vec![],
                    run,
                    ..self.journal_record(scope, ChangeKind::Conflict, note)
                }
            }
        };
        batch.report.changes.push(record.id.clone());
        batch.ops.push(StoreOp::PutJournal(Box::new(record)));
        Ok(())
    }

    pub(super) fn do_apply_changes(
        &self,
        who: &Accessor,
        set: &ChangeSet,
    ) -> Result<ChangeReport, MemoryError> {
        require_owner_or_guardian(who, "zmiany konsolidacji")?;
        validate_scope(&set.scope)?;
        let mut batch = Batch {
            entries: self
                .backend
                .entries(&set.scope)?
                .into_iter()
                .map(|e| (e.id.clone(), e))
                .collect(),
            ops: Vec::new(),
            report: ChangeReport::default(),
        };
        for op in &set.ops {
            self.apply_op(who, set, &mut batch, op)?;
        }
        self.commit(&set.scope, batch.ops)?;
        let r = &batch.report;
        self.emit(
            names::CHANGES_APPLIED,
            None,
            json!({ "scope": scope_key(&set.scope), "run": set.run, "changes": r.changes.len(),
                    "created": r.created.len(), "superseded": r.superseded.len(),
                    "expired": r.expired.len(), "conflicts": r.conflicts }),
        );
        Ok(batch.report)
    }
}
