//! Eksport i import zakresu (paczka `.alfa`, kategoria `memory`; adapter `DocumentStore` w
//! `memory-impl`). Wektory i FTS nie są eksportowane — import buduje indeks na nowo.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use super::{MAX_TEXT_CHARS, MemoryEngine};
use crate::access::{Accessor, require_owner};
use crate::backend::{MemoryBackend, StoreOp};
use crate::cascade::plan_cascade;
use crate::error::MemoryError;
use crate::events as names;
use crate::model::{EntryRef, is_broader, scope_key, validate_scope};
use crate::service::{ImportPolicy, ImportReport};
use crate::types::{MemoryEntry, MemoryScope};

/// Maksymalna długość identyfikatora wpisu z importu.
pub const MAX_ID_LEN: usize = 128;

/// Sprawdza wpis z paczki; zwraca wpis znormalizowany (zaufanie liczone z proweniencji).
pub fn check_imported(scope: &MemoryScope, mut e: MemoryEntry) -> Result<MemoryEntry, String> {
    if &e.scope != scope {
        return Err("zakres wpisu różny od zakresu dokumentu".into());
    }
    let id_ok = !e.id.0.is_empty()
        && e.id.0.len() <= MAX_ID_LEN
        && e.id
            .0
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !id_ok {
        return Err("nieprawidłowy identyfikator".into());
    }
    if e.text.trim().is_empty() || e.text.chars().count() > MAX_TEXT_CHARS {
        return Err("nieprawidłowa treść".into());
    }
    if !(0.0..=1.0).contains(&e.confidence) {
        return Err("pewność poza 0–1".into());
    }
    e.trusted = e.provenance.is_trusted();
    if !e.trusted && !matches!(scope, MemoryScope::Session(_)) {
        return Err("treść niezaufana w zakresie szerszym niż sesja".into());
    }
    if e.origin
        .derived_from
        .iter()
        .any(|r| is_broader(&r.scope, scope))
    {
        return Err("źródło w zakresie szerszym niż wpis".into());
    }
    Ok(e)
}

impl<B: MemoryBackend> MemoryEngine<B> {
    pub(super) fn do_export(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
        name: &str,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        require_owner(who, "eksport pamięci")?;
        validate_scope(scope)?;
        let entries = self.backend.entries(scope)?;
        if !entries.is_empty() {
            self.backend.commit(
                scope,
                vec![StoreOp::NoteExport {
                    name: name.to_owned(),
                    at: self.now(),
                }],
            )?;
        }
        self.emit(
            names::EXPORTED,
            None,
            json!({ "scope": scope_key(scope), "entries": entries.len() }),
        );
        Ok(entries)
    }

    pub(super) fn do_import(
        &self,
        who: &Accessor,
        scope: &MemoryScope,
        incoming: Vec<MemoryEntry>,
        policy: ImportPolicy,
    ) -> Result<ImportReport, MemoryError> {
        require_owner(who, "import pamięci")?;
        validate_scope(scope)?;
        let local: BTreeMap<_, _> = self
            .backend
            .entries(scope)?
            .into_iter()
            .map(|e| (e.id.clone(), e))
            .collect();
        let mut report = ImportReport::default();
        let mut ops = Vec::new();
        let mut seen = BTreeSet::new();
        for e in incoming {
            let id = e.id.0.clone();
            if !seen.insert(id.clone()) {
                report
                    .rejected
                    .push((id, "powtórzony identyfikator".into()));
                continue;
            }
            match check_imported(scope, e) {
                Ok(e) => {
                    match local.get(&e.id) {
                        Some(old) if *old == e => {}
                        Some(_) => report.replaced += 1,
                        None => report.added += 1,
                    }
                    ops.push(StoreOp::Put(Box::new(e)));
                }
                Err(reason) => report.rejected.push((id, reason)),
            }
        }
        self.commit(scope, ops)?;
        if policy == ImportPolicy::Replace {
            let seeds: BTreeSet<EntryRef> = local
                .values()
                .filter(|e| !seen.contains(&e.id.0))
                .map(MemoryEntry::entry_ref)
                .collect();
            if !seeds.is_empty() {
                let mut scopes = vec![scope.clone()];
                scopes.extend(self.broader_scopes()?.into_iter().filter(|s| s != scope));
                let mut all = Vec::new();
                for s in &scopes {
                    all.extend(self.backend.entries(s)?);
                }
                let plan = plan_cascade(&all, &seeds, false);
                report.removed = self.execute_plan(&plan, &all, &scopes, None)?;
            }
        }
        self.emit(
            names::IMPORTED,
            None,
            json!({ "scope": scope_key(scope), "added": report.added, "replaced": report.replaced,
                    "rejected": report.rejected.len(), "removed": report.removed.removed.len() }),
        );
        Ok(report)
    }
}
