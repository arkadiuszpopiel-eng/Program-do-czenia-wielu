//! Zapis: `remember`, sprzeczności tematu (nowa wersja), przypięcie, zatwierdzenie, awans, edycja.

use serde_json::json;

use super::{MemoryEngine, normalized_text, subject_key, validate_f7};
use crate::access::{Accessor, Op, authorize, require_owner};
use crate::backend::{MemoryBackend, StoreOp};
use crate::error::MemoryError;
use crate::events as names;
use crate::inspect::EntryEdit;
use crate::journal::{ChangeKind, JournalRecord};
use crate::model::{
    Derivation, EntryRef, EntryState, Origin, SupersedeReason, Supersession, entry_state,
    is_broader, scope_key, scope_session, validate_scope,
};
use crate::types::{
    Layer, MemoryEntry, MemoryId, MemoryScope, NewMemory, Provenance, RememberMode,
};

/// Ranga zaufania proweniencji (nowsza wersja zastępuje starszą tylko przy randze ≥).
pub fn trust_rank(p: &Provenance) -> u8 {
    match p {
        Provenance::User => 3,
        Provenance::Agent { .. } | Provenance::Import { .. } => 2,
        Provenance::UntrustedContent { .. } => 0,
    }
}

impl<B: MemoryBackend> MemoryEngine<B> {
    pub(super) fn do_remember(
        &self,
        who: &Accessor,
        mut new: NewMemory,
        mode: RememberMode,
    ) -> Result<MemoryEntry, MemoryError> {
        validate_f7(&new)?;
        authorize(who, &new.scope, Op::Write)?;
        match who {
            Accessor::Guardian => {
                return Err(MemoryError::forbidden(
                    "Strażniczka zapisuje wyłącznie przez zmiany konsolidacji",
                ));
            }
            Accessor::Agent(_) if new.provenance == Provenance::User => {
                return Err(MemoryError::forbidden(
                    "agentka nie może nadać proweniencji użytkownika",
                ));
            }
            _ => {}
        }
        if mode == RememberMode::AutoPendingApproval && !new.provenance.is_trusted() {
            return Err(MemoryError::UntrustedAutoRemember);
        }
        if matches!(who, Accessor::Agent(_)) {
            for r in &new.origin.derived_from {
                authorize(who, &r.scope, Op::Read)?;
            }
        }
        if new.origin.session.is_none() {
            new.origin.session = match (who, &new.scope) {
                (_, MemoryScope::Session(s)) => Some(s.clone()),
                (Accessor::Agent(a), _) => Some(a.session.clone()),
                _ => None,
            };
        }
        self.check_flow(&new)?;
        let in_session = matches!(new.scope, MemoryScope::Session(_));
        let approved = mode == RememberMode::Explicit && (who.is_owner() || in_session);
        let scope = new.scope.clone();
        let mut entry =
            MemoryEntry::from_new(MemoryId(self.new_id("mem")), new, self.now(), approved);
        let mut ops = if approved {
            self.resolve_subject(&mut entry)?
        } else {
            Vec::new()
        };
        ops.insert(0, StoreOp::Put(Box::new(entry.clone())));
        self.commit(&scope, ops)?;
        let kind = if entry.approved {
            names::REMEMBERED
        } else {
            names::PENDING_APPROVAL
        };
        self.emit(
            kind,
            scope_session(&scope),
            json!({ "memory": entry.id, "scope": scope_key(&scope), "layer": entry.layer,
                    "trusted": entry.trusted, "approved": entry.approved }),
        );
        Ok(entry)
    }

    /// Sprzeczność tematu przy zapisie/zatwierdzeniu: aktywny fakt o tym samym temacie i innej
    /// treści → zastąpienie (nowa wersja z odwołaniem), gdy zaufanie nowego ≥ starego; inaczej
    /// konflikt w dzienniku (decyzja użytkownika). Zmienia `entry` (wersja, `supersedes`).
    pub(super) fn resolve_subject(
        &self,
        entry: &mut MemoryEntry,
    ) -> Result<Vec<StoreOp>, MemoryError> {
        let Some(subject) = entry.subject.as_deref().map(subject_key) else {
            return Ok(Vec::new());
        };
        if entry.layer != Layer::Semantic || subject.is_empty() {
            return Ok(Vec::new());
        }
        let now = self.now();
        let text = normalized_text(&entry.text);
        let rivals: Vec<MemoryEntry> = self
            .backend
            .entries(&entry.scope)?
            .into_iter()
            .filter(|o| {
                o.id != entry.id
                    && o.layer == Layer::Semantic
                    && entry_state(o, now) == EntryState::Active
                    && o.subject.as_deref().map(subject_key).as_deref() == Some(subject.as_str())
                    && normalized_text(&o.text) != text
            })
            .collect();
        let (wins, conflicts): (Vec<MemoryEntry>, Vec<MemoryEntry>) =
            rivals.into_iter().partition(|old| {
                entry.trusted && trust_rank(&entry.provenance) >= trust_rank(&old.provenance)
            });
        for old in &wins {
            entry.supersedes = Some(old.id.clone());
            entry.version = entry.version.max(old.version + 1);
        }
        let mut ops = Vec::new();
        for old in wins {
            let mut marked = old.clone();
            marked.superseded = Some(Supersession {
                by: entry.id.clone(),
                reason: SupersedeReason::Contradiction,
                at: now,
            });
            let record = JournalRecord {
                refs: vec![old.id.clone(), entry.id.clone()],
                before: vec![old],
                after: vec![marked.clone(), entry.clone()],
                run: None,
                ..self.journal_record(
                    &entry.scope,
                    ChangeKind::Supersede,
                    "sprzeczność tematu: nowa wersja",
                )
            };
            ops.push(StoreOp::Put(Box::new(marked)));
            ops.push(StoreOp::PutJournal(Box::new(record)));
        }
        for old in conflicts {
            let record = JournalRecord {
                refs: vec![old.id.clone(), entry.id.clone()],
                before: Vec::new(),
                after: Vec::new(),
                run: None,
                ..self.journal_record(
                    &entry.scope,
                    ChangeKind::Conflict,
                    "sprzeczność tematu bez rozstrzygnięcia (niższe zaufanie)",
                )
            };
            ops.push(StoreOp::PutJournal(Box::new(record)));
        }
        Ok(ops)
    }

    pub(super) fn do_set_pinned(
        &self,
        who: &Accessor,
        r: &EntryRef,
        pinned: bool,
    ) -> Result<MemoryEntry, MemoryError> {
        authorize(who, &r.scope, Op::Write)?;
        let mut entry = self.load(r)?;
        if entry.superseded.is_some() {
            return Err(MemoryError::conflict(
                "wpis zastąpiony — przypnij nowszą wersję",
            ));
        }
        if entry.pinned != pinned {
            entry.pinned = pinned;
            self.commit(&r.scope, vec![StoreOp::Put(Box::new(entry.clone()))])?;
            self.emit(
                names::PINNED,
                scope_session(&r.scope),
                json!({ "memory": r.id, "pinned": pinned }),
            );
        }
        Ok(entry)
    }

    pub(super) fn do_approve(
        &self,
        who: &Accessor,
        r: &EntryRef,
    ) -> Result<MemoryEntry, MemoryError> {
        require_owner(who, "zatwierdzenie wpisu")?;
        let mut entry = self.load(r)?;
        if entry.approved {
            return Ok(entry);
        }
        entry.approved = true;
        let mut ops = self.resolve_subject(&mut entry)?;
        ops.insert(0, StoreOp::Put(Box::new(entry.clone())));
        self.commit(&r.scope, ops)?;
        self.emit(
            names::APPROVED,
            scope_session(&r.scope),
            json!({ "memory": r.id, "scope": scope_key(&r.scope) }),
        );
        Ok(entry)
    }

    pub(super) fn do_promote(
        &self,
        who: &Accessor,
        r: &EntryRef,
        to: MemoryScope,
    ) -> Result<MemoryEntry, MemoryError> {
        validate_scope(&to)?;
        authorize(who, &r.scope, Op::Read)?;
        let src = self.load(r)?;
        if !src.trusted || !src.provenance.is_trusted() {
            return Err(MemoryError::UntrustedCannotPromote);
        }
        if !is_broader(&to, &src.scope) {
            return Err(MemoryError::invalid(
                "awans tylko do zakresu szerszego niż bieżący",
            ));
        }
        if entry_state(&src, self.now()) != EntryState::Active {
            return Err(MemoryError::conflict("awansować można tylko wpis aktywny"));
        }
        let approved = match who {
            Accessor::Owner => true,
            Accessor::Guardian => false,
            Accessor::Agent(_) => {
                authorize(who, &to, Op::Write)?;
                false
            }
        };
        let origin = Origin {
            session: src
                .origin
                .session
                .clone()
                .or_else(|| scope_session(&src.scope).cloned()),
            turn: src.origin.turn,
            derived_from: vec![r.clone()],
            derivation: Some(Derivation::Promoted),
        };
        let new = NewMemory {
            scope: to.clone(),
            layer: src.layer,
            text: src.text.clone(),
            entities: src.entities.clone(),
            provenance: src.provenance.clone(),
            confidence: src.confidence,
            ttl_secs: src.ttl_secs,
            subject: src.subject.clone(),
            origin,
        };
        self.check_flow(&new)?;
        if let Some(existing) =
            self.backend.entries(&to)?.into_iter().find(|e| {
                e.superseded.is_none() && e.origin.derived_from == new.origin.derived_from
            })
        {
            return Ok(existing);
        }
        let entry = MemoryEntry::from_new(MemoryId(self.new_id("mem")), new, self.now(), approved);
        let record = JournalRecord {
            refs: vec![entry.id.clone(), src.id.clone()],
            before: Vec::new(),
            after: vec![entry.clone()],
            run: None,
            ..self.journal_record(
                &to,
                ChangeKind::Promote,
                format!("awans z {}", scope_key(&src.scope)),
            )
        };
        self.commit(
            &to,
            vec![
                StoreOp::Put(Box::new(entry.clone())),
                StoreOp::PutJournal(Box::new(record)),
            ],
        )?;
        self.emit(
            names::PROMOTED,
            None,
            json!({ "memory": entry.id, "from": scope_key(&src.scope), "to": scope_key(&to),
                    "approved": approved }),
        );
        Ok(entry)
    }

    pub(super) fn do_edit(
        &self,
        who: &Accessor,
        r: &EntryRef,
        edit: &EntryEdit,
    ) -> Result<MemoryEntry, MemoryError> {
        require_owner(who, "edycja wpisu")?;
        if edit.is_empty() {
            return Err(MemoryError::invalid("pusta edycja"));
        }
        let old = self.load(r)?;
        if old.superseded.is_some() {
            return Err(MemoryError::conflict(
                "edytować można tylko aktualną wersję",
            ));
        }
        let provenance = if old.provenance.is_trusted() {
            Provenance::User
        } else {
            old.provenance.clone()
        };
        let new = NewMemory {
            scope: old.scope.clone(),
            layer: old.layer,
            text: edit.text.clone().unwrap_or_else(|| old.text.clone()),
            entities: edit
                .entities
                .clone()
                .unwrap_or_else(|| old.entities.clone()),
            provenance,
            confidence: edit.confidence.unwrap_or(old.confidence),
            ttl_secs: edit.ttl_secs.unwrap_or(old.ttl_secs),
            subject: edit.subject.clone().unwrap_or_else(|| old.subject.clone()),
            origin: Origin {
                session: old.origin.session.clone(),
                turn: old.origin.turn,
                derived_from: vec![r.clone()],
                derivation: Some(Derivation::Edited),
            },
        };
        validate_f7(&new)?;
        let now = self.now();
        let mut entry = MemoryEntry::from_new(MemoryId(self.new_id("mem")), new, now, true);
        entry.pinned = old.pinned;
        entry.version = old.version + 1;
        entry.supersedes = Some(old.id.clone());
        let mut marked = old.clone();
        marked.superseded = Some(Supersession {
            by: entry.id.clone(),
            reason: SupersedeReason::Edit,
            at: now,
        });
        let record = JournalRecord {
            refs: vec![old.id.clone(), entry.id.clone()],
            before: vec![old],
            after: vec![marked.clone(), entry.clone()],
            run: None,
            ..self.journal_record(&r.scope, ChangeKind::Edit, "edycja w Inspektorze")
        };
        self.commit(
            &r.scope,
            vec![
                StoreOp::Put(Box::new(marked)),
                StoreOp::Put(Box::new(entry.clone())),
                StoreOp::PutJournal(Box::new(record)),
            ],
        )?;
        self.emit(
            names::EDITED,
            scope_session(&r.scope),
            json!({ "memory": entry.id, "previous": r.id, "version": entry.version }),
        );
        Ok(entry)
    }
}
