//! Zamykanie kroków, podsumowania („Delta: przeniesiono 14 plików”), retencja i limit magazynu.

use std::collections::BTreeSet;

use core_bus_contract::SessionId;

use crate::engine::{Journal, StepRec};
use crate::types::{
    BlobId, JournalRecord, OpCounts, StepId, StepSummary, UndoError, UndoOp, UndoReport,
};

/// Polska odmiana „plik”.
pub fn files_pl(n: u32) -> String {
    let word = if n == 1 {
        "plik"
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        "pliki"
    } else {
        "plików"
    };
    format!("{n} {word}")
}

fn blobs_of(s: &StepRec) -> Vec<BlobId> {
    let mut out = Vec::new();
    for e in &s.entries {
        match &e.op {
            UndoOp::Write { pre_image, .. }
            | UndoOp::Delete { pre_image, .. }
            | UndoOp::DeletePermanent { pre_image, .. } => out.extend(pre_image.clone()),
            UndoOp::ScopeSnapshot { files, .. } => {
                out.extend(files.values().map(|(_, b)| b.clone()))
            }
            UndoOp::Copy { .. } | UndoOp::Move { .. } => {}
        }
    }
    out
}

fn summarize(step: StepId, s: &StepRec) -> StepSummary {
    let mut c = OpCounts::default();
    let mut reversible = true;
    for e in &s.entries {
        match &e.op {
            UndoOp::Write {
                before, pre_image, ..
            } => {
                c.written += 1;
                reversible &= matches!(before, crate::FileState::Absent) || pre_image.is_some();
            }
            UndoOp::Copy { .. } => c.copied += 1,
            UndoOp::Move { .. } => c.moved += 1,
            UndoOp::Delete { .. } => c.deleted += 1,
            UndoOp::DeletePermanent { pre_image, .. } => {
                c.purged += 1;
                reversible &= pre_image.is_some();
            }
            UndoOp::ScopeSnapshot { .. } => c.snapshots += 1,
        }
    }
    let mut parts = Vec::new();
    for (n, verb) in [
        (c.written, "zapisano"),
        (c.copied, "skopiowano"),
        (c.moved, "przeniesiono"),
        (c.deleted, "przeniesiono do Kosza"),
        (c.purged, "trwale usunięto"),
    ] {
        if n > 0 {
            parts.push(format!("{verb} {}", files_pl(n)));
        }
    }
    if c.snapshots > 0 {
        parts.push("polecenie w zakresie ze snapshotem".into());
    }
    let who = s.ctx.agent.as_ref().map_or("Alfa", |a| a.as_str());
    let what = if parts.is_empty() {
        s.ctx.label.clone()
    } else {
        parts.join(", ")
    };
    StepSummary {
        step,
        ctx: s.ctx.clone(),
        counts: c,
        reversible: reversible && !s.expired && !s.undone,
        committed: s.committed_ms.is_some(),
        undone: s.undone,
        expired: s.expired,
        text: format!("{who}: {what}"),
    }
}

impl Journal {
    /// Zatwierdza krok (zapamiętuje stan „po” zakresów shella).
    pub fn commit_step(&self, step: StepId) -> Result<StepSummary, UndoError> {
        self.open_ctx(step)?;
        let now = self.clock.now_ms();
        let roots: Vec<(u32, std::path::PathBuf)> = {
            let st = self.lock();
            st.steps
                .get(&step)
                .map(|s| {
                    s.entries
                        .iter()
                        .filter_map(|e| match &e.op {
                            UndoOp::ScopeSnapshot { root, .. } => Some((e.seq, root.clone())),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut posts = std::collections::BTreeMap::new();
        for (seq, root) in roots {
            posts.insert(seq, self.hashes(&root)?);
        }
        let rec = JournalRecord::Commit {
            step,
            at_ms: now,
            posts: posts.clone(),
        };
        self.store.append(&rec).map_err(UndoError::Store)?;
        let mut st = self.lock();
        let s = st
            .steps
            .get_mut(&step)
            .ok_or(UndoError::UnknownStep(step))?;
        s.committed_ms = Some(now);
        s.posts = posts;
        Ok(summarize(step, s))
    }

    /// Podsumowanie kroku.
    pub fn summary(&self, step: StepId) -> Option<StepSummary> {
        self.lock().steps.get(&step).map(|s| summarize(step, s))
    }

    /// Kroki sesji (rosnąco).
    pub fn steps(&self, session: &SessionId) -> Vec<StepSummary> {
        self.lock()
            .steps
            .iter()
            .filter(|(_, s)| s.ctx.session == *session)
            .map(|(id, s)| summarize(*id, s))
            .collect()
    }

    /// Cofa `n` ostatnich zatwierdzonych, niecofniętych kroków sesji (od najnowszego);
    /// zatrzymuje się na pierwszym błędzie (np. konflikt).
    pub fn undo_last(&self, session: &SessionId, n: usize) -> Result<Vec<UndoReport>, UndoError> {
        let ids: Vec<StepId> = {
            let st = self.lock();
            st.steps
                .iter()
                .rev()
                .filter(|(_, s)| {
                    s.ctx.session == *session && s.committed_ms.is_some() && !s.undone && !s.expired
                })
                .take(n)
                .map(|(id, _)| *id)
                .collect()
        };
        ids.into_iter().map(|id| self.undo(id)).collect()
    }

    fn expire_steps(&self, ids: &[StepId]) {
        let mut st = self.lock();
        let mut dropped: BTreeSet<BlobId> = BTreeSet::new();
        for id in ids {
            if let Some(s) = st.steps.get_mut(id) {
                s.expired = true;
                dropped.extend(blobs_of(s));
                let _ = self.store.append(&JournalRecord::Expired { step: *id });
            }
        }
        let live: BTreeSet<BlobId> = st
            .steps
            .values()
            .filter(|s| !s.expired)
            .flat_map(blobs_of)
            .collect();
        for b in dropped.difference(&live) {
            let _ = self.store.delete_blob(b);
        }
    }

    /// Retencja: kroki starsze niż `retention_ms` (od zatwierdzenia) oraz cofnięte tracą
    /// pre-image. Zwraca liczbę przeterminowanych kroków.
    pub fn prune(&self) -> usize {
        let now = self.clock.now_ms();
        let ids: Vec<StepId> = self
            .lock()
            .steps
            .iter()
            .filter(|(_, s)| !s.expired)
            .filter(|(_, s)| {
                s.undone
                    || s.committed_ms
                        .is_some_and(|t| now.saturating_sub(t) >= self.limits.retention_ms)
            })
            .map(|(id, _)| *id)
            .collect();
        self.expire_steps(&ids);
        ids.len()
    }

    /// Zwalnia miejsce w magazynie: przeterminowuje najstarsze zatwierdzone kroki.
    pub(crate) fn make_room(&self, needed: u64) {
        loop {
            if self.store.blob_bytes().saturating_add(needed) <= self.limits.store_limit_bytes {
                return;
            }
            let oldest = self
                .lock()
                .steps
                .iter()
                .find(|(_, s)| !s.expired && (s.committed_ms.is_some() || s.undone))
                .map(|(id, _)| *id);
            match oldest {
                Some(id) => self.expire_steps(&[id]),
                None => return,
            }
        }
    }

    /// Początek kroku (ms) — do UI osi czasu.
    pub fn started_ms(&self, step: StepId) -> Option<u64> {
        self.lock().steps.get(&step).map(|s| s.started_ms)
    }
}
