//! Propozycje awansu do pamięci globalnej i cofnięcie przebiegu.

use std::collections::{BTreeMap, BTreeSet};

use memory_contract::{
    Accessor, EntryState, Layer, MemoryEntry, MemoryError, MemoryScope, MemoryService, entry_state,
    normalized_text,
};

use crate::guardian::Guardian;
use crate::report::RunReport;

impl Guardian {
    /// Fakty powtarzające się w ≥ N sesjach (nieprywatnych, zaufane, aktywne) → kopia oczekująca w
    /// pamięci globalnej (awans tylko za zgodą użytkownika; silnik odrzuca sesje prywatne).
    pub(crate) fn propose_promotions(&self, report: &mut RunReport) {
        let memory = &self.ports.memory;
        let Ok(scopes) = memory.scopes(&Accessor::Guardian) else {
            return;
        };
        let now = self.ports.clock.now();
        let mut groups: BTreeMap<String, Vec<MemoryEntry>> = BTreeMap::new();
        let mut global: BTreeSet<String> = BTreeSet::new();
        for summary in scopes {
            let Ok(entries) = self.entries(&summary.scope) else {
                continue;
            };
            match &summary.scope {
                MemoryScope::Global => {
                    global.extend(entries.iter().map(|e| normalized_text(&e.text)))
                }
                MemoryScope::Session(s) if !self.ports.privacy.is_private(s) => {
                    for e in entries.into_iter().filter(|e| {
                        e.layer == Layer::Semantic
                            && e.trusted
                            && entry_state(e, now) == EntryState::Active
                    }) {
                        groups.entry(normalized_text(&e.text)).or_default().push(e);
                    }
                }
                _ => {}
            }
        }
        for (text, group) in groups {
            let sessions: BTreeSet<_> = group.iter().map(|e| e.scope.clone()).collect();
            if sessions.len() < self.config.promotion_min_sessions || global.contains(&text) {
                continue;
            }
            if memory
                .promote_as(
                    &Accessor::Guardian,
                    &group[0].entry_ref(),
                    MemoryScope::Global,
                )
                .is_ok()
            {
                report.proposals += 1;
            }
        }
    }
}

/// Cofa cały przebieg (właściciel): zmiany z dziennika z `run`, od najnowszej; wygaszenia są
/// nieodwracalne i zostają pominięte. Zwraca liczbę cofniętych zmian i pominiętych.
pub fn undo_run(memory: &dyn MemoryService, run: &str) -> Result<(usize, usize), MemoryError> {
    let mut records = Vec::new();
    for summary in memory.scopes(&Accessor::Owner)? {
        for record in memory.journal(&Accessor::Owner, &summary.scope)? {
            if record.run.as_deref() == Some(run) && !record.undone {
                records.push(record);
            }
        }
    }
    records.sort_by(|a, b| (b.at, &b.id).cmp(&(a.at, &a.id)));
    let (mut undone, mut skipped) = (0, 0);
    for record in records {
        match memory.undo(&Accessor::Owner, &record.scope, &record.id) {
            Ok(_) => undone += 1,
            Err(MemoryError::Conflict { .. } | MemoryError::Invalid { .. }) => skipped += 1,
            Err(e) => return Err(e),
        }
    }
    Ok((undone, skipped))
}
