//! Odczyt: `recall` (hybryda magazynu + reranking), zestaw roboczy, Inspektor, „dlaczego”.

use std::collections::BTreeMap;

use core_bus_contract::SessionId;
use lib_sqlstore::search_tokens;
use serde_json::json;

use super::MemoryEngine;
use crate::access::{Accessor, Op, authorize, readable_scopes, require_owner_or_guardian};
use crate::backend::{CandidateQuery, MemoryBackend};
use crate::error::MemoryError;
use crate::events as names;
use crate::inspect::{Explanation, InspectorItem, InspectorPage, InspectorQuery, SourceLink};
use crate::model::{
    EntryRef, EntryState, SupersedeReason, entry_state, expires_at, is_recallable, scope_key,
    validate_scope,
};
use crate::rerank::{RerankItem, content_stems, stem_coverage};
use crate::service::{RecallRequest, ScopeSummary, WorkingSet};
use crate::types::{Layer, MemoryEntry, MemoryScope, Recalled};

/// Liczba kandydatów z magazynu na zakres: `max(4·k, 30)`.
pub fn candidate_limit(k: usize) -> usize {
    (k.saturating_mul(4)).max(30)
}

impl<B: MemoryBackend> MemoryEngine<B> {
    fn recall_scopes(
        &self,
        who: &Accessor,
        requested: &[MemoryScope],
    ) -> Result<Vec<MemoryScope>, MemoryError> {
        let scopes = if requested.is_empty() {
            match who {
                Accessor::Agent(a) => readable_scopes(a),
                _ => return Err(MemoryError::invalid("podaj zakresy `recall`")),
            }
        } else {
            let mut out: Vec<MemoryScope> = Vec::new();
            for s in requested {
                if !out.contains(s) {
                    out.push(s.clone());
                }
            }
            out
        };
        for s in &scopes {
            validate_scope(s)?;
            authorize(who, s, Op::Read)?;
        }
        Ok(scopes)
    }

    pub(super) fn do_recall(
        &self,
        who: &Accessor,
        req: &RecallRequest,
    ) -> Result<Vec<Recalled>, MemoryError> {
        let scopes = self.recall_scopes(who, &req.scopes)?;
        if req.k == 0 || search_tokens(&req.query).is_empty() {
            return Ok(Vec::new());
        }
        let now = self.now();
        let key = format!(
            "{}|{}|{}|{}|{:?}",
            who.cache_key(),
            scopes.iter().map(scope_key).collect::<Vec<_>>().join(","),
            req.query,
            req.k,
            req.layers
        );
        if let Some(hits) = self.cache.get(&key, now) {
            return Ok(hits);
        }
        let query = CandidateQuery {
            text: req.query.clone(),
            stems: content_stems(&req.query),
            limit: candidate_limit(req.k),
        };
        let mut items: Vec<(MemoryEntry, f32)> = Vec::new();
        for scope in &scopes {
            for cand in self.backend.search(scope, &query)? {
                let Some(entry) = self.backend.entry(scope, &cand.id)? else {
                    continue;
                };
                let layer_ok = if req.layers.is_empty() {
                    entry.layer != Layer::Working
                } else {
                    req.layers.contains(&entry.layer)
                };
                if layer_ok && is_recallable(&entry, now) {
                    items.push((entry, cand.score));
                }
            }
        }
        let rerank: Vec<RerankItem<'_>> = items
            .iter()
            .map(|(entry, retrieval)| RerankItem {
                entry,
                retrieval: *retrieval,
            })
            .collect();
        let scores = self.ports.reranker.rerank(&req.query, &rerank, now);
        let mut out: Vec<Recalled> = items
            .into_iter()
            .zip(scores)
            .map(|((entry, _), score)| Recalled { entry, score })
            .collect();
        out.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.entry.entry_ref().cmp(&b.entry.entry_ref()))
        });
        out.truncate(req.k);
        self.emit(
            names::RECALLED,
            None,
            json!({ "scopes": scopes.len(), "results": out.len() }),
        );
        self.cache.put(key, now, out.clone());
        Ok(out)
    }

    pub(super) fn do_get(&self, who: &Accessor, r: &EntryRef) -> Result<MemoryEntry, MemoryError> {
        validate_scope(&r.scope)?;
        authorize(who, &r.scope, Op::Read)?;
        self.load(r)
    }

    pub(super) fn do_working_set(
        &self,
        who: &Accessor,
        session: &SessionId,
        query: Option<&str>,
        budget_chars: usize,
    ) -> Result<WorkingSet, MemoryError> {
        let scopes = match who {
            Accessor::Agent(a) if &a.session == session => readable_scopes(a),
            Accessor::Agent(_) => {
                return Err(MemoryError::forbidden(
                    "zestaw roboczy tylko dla własnej sesji agentki",
                ));
            }
            _ => vec![MemoryScope::Session(session.clone()), MemoryScope::Global],
        };
        let now = self.now();
        let mut set = WorkingSet::default();
        for scope in &scopes {
            for e in self.backend.entries(scope)? {
                if e.pinned && is_recallable(&e, now) {
                    set.pinned.push(e);
                }
            }
        }
        set.pinned
            .sort_by(|a, b| (a.created_at, a.entry_ref()).cmp(&(b.created_at, b.entry_ref())));
        let mut kept = Vec::new();
        for e in std::mem::take(&mut set.pinned) {
            let len = e.text.chars().count();
            if set.chars + len > budget_chars {
                set.truncated = true;
                continue;
            }
            set.chars += len;
            kept.push(e);
        }
        set.pinned = kept;
        if let Some(q) = query.filter(|q| !q.trim().is_empty()) {
            let req = RecallRequest::new(scopes, q, 10);
            for hit in self.do_recall(who, &req)? {
                if hit.entry.pinned {
                    continue;
                }
                let len = hit.entry.text.chars().count();
                if set.chars + len > budget_chars {
                    set.truncated = true;
                    continue;
                }
                set.chars += len;
                set.recalled.push(hit);
            }
        }
        Ok(set)
    }

    pub(super) fn do_inspect(
        &self,
        who: &Accessor,
        q: &InspectorQuery,
    ) -> Result<InspectorPage, MemoryError> {
        require_owner_or_guardian(who, "Inspektor pamięci")?;
        let scopes = if q.scopes.is_empty() {
            self.backend.scopes()?
        } else {
            q.scopes.clone()
        };
        let now = self.now();
        let text = q.text.as_deref().filter(|t| !content_stems(t).is_empty());
        let query_stems = text.map(content_stems).unwrap_or_default();
        let mut items: Vec<InspectorItem> = Vec::new();
        for scope in &scopes {
            validate_scope(scope)?;
            let scores: Option<BTreeMap<String, f32>> = match text {
                Some(t) => {
                    let cq = CandidateQuery {
                        text: t.to_owned(),
                        stems: content_stems(t),
                        limit: 500,
                    };
                    Some(
                        self.backend
                            .search(scope, &cq)?
                            .into_iter()
                            .map(|c| (c.id.0, c.score))
                            .collect(),
                    )
                }
                None => None,
            };
            for entry in self.backend.entries(scope)? {
                if !q.accepts(&entry, now) {
                    continue;
                }
                if text.is_some() && stem_coverage(&query_stems, &content_stems(&entry.text)) <= 0.0
                {
                    continue;
                }
                let score = match &scores {
                    Some(map) => match map.get(&entry.id.0) {
                        Some(s) => Some(*s),
                        None => continue,
                    },
                    None => None,
                };
                items.push(InspectorItem {
                    state: entry_state(&entry, now),
                    entry,
                    score,
                });
            }
        }
        items.sort_by(|a, b| {
            let by_score = b.score.unwrap_or(0.0).total_cmp(&a.score.unwrap_or(0.0));
            by_score
                .then_with(|| b.entry.created_at.cmp(&a.entry.created_at))
                .then_with(|| a.entry.entry_ref().cmp(&b.entry.entry_ref()))
        });
        let total = items.len();
        let items = items
            .into_iter()
            .skip(q.offset)
            .take(q.page_size())
            .collect();
        Ok(InspectorPage { items, total })
    }

    pub(super) fn do_explain(
        &self,
        who: &Accessor,
        r: &EntryRef,
    ) -> Result<Explanation, MemoryError> {
        require_owner_or_guardian(who, "„dlaczego to pamiętam”")?;
        let entry = self.load(r)?;
        let now = self.now();
        let scope_entries = self.backend.entries(&r.scope)?;
        let by_id: BTreeMap<&str, &MemoryEntry> =
            scope_entries.iter().map(|e| (e.id.0.as_str(), e)).collect();
        let mut sources = Vec::new();
        for src in &entry.origin.derived_from {
            let found = self.backend.entry(&src.scope, &src.id)?;
            sources.push(SourceLink {
                entry: src.clone(),
                exists: found.is_some(),
                state: found.map(|e| entry_state(&e, now)),
            });
        }
        let mut versions = vec![entry.clone()];
        let mut cursor = entry.supersedes.clone();
        while let Some(prev) = cursor.and_then(|id| by_id.get(id.0.as_str()).copied()) {
            if versions.iter().any(|v| v.id == prev.id) {
                break;
            }
            versions.insert(0, prev.clone());
            cursor = prev.supersedes.clone();
        }
        let mut next = entry.superseded.as_ref().map(|s| s.by.clone());
        while let Some(n) = next.and_then(|id| by_id.get(id.0.as_str()).copied()) {
            if versions.iter().any(|v| v.id == n.id) {
                break;
            }
            versions.push(n.clone());
            next = n.superseded.as_ref().map(|s| s.by.clone());
        }
        let merged = scope_entries
            .iter()
            .filter(|e| {
                e.superseded
                    .as_ref()
                    .is_some_and(|s| s.by == entry.id && s.reason == SupersedeReason::Duplicate)
            })
            .map(MemoryEntry::entry_ref)
            .collect();
        let mut derived = Vec::new();
        let mut scan = vec![r.scope.clone()];
        scan.extend(self.broader_scopes()?.into_iter().filter(|s| *s != r.scope));
        for scope in &scan {
            for e in self.backend.entries(scope)? {
                if e.origin.derived_from.contains(r) {
                    derived.push(e.entry_ref());
                }
            }
        }
        let journal = self
            .backend
            .journal(&r.scope)?
            .into_iter()
            .filter(|j| j.touches(&entry.id))
            .collect();
        let state = entry_state(&entry, now);
        Ok(Explanation {
            reasons: super::explain::reasons(&entry, state, &sources, versions.len()),
            expires_at: expires_at(&entry),
            state,
            entry,
            sources,
            versions,
            merged,
            derived,
            journal,
        })
    }

    pub(super) fn do_scopes(&self, who: &Accessor) -> Result<Vec<ScopeSummary>, MemoryError> {
        require_owner_or_guardian(who, "lista zakresów pamięci")?;
        let now = self.now();
        let mut out = Vec::new();
        for scope in self.backend.scopes()? {
            let entries = self.backend.entries(&scope)?;
            let count =
                |st: EntryState| entries.iter().filter(|e| entry_state(e, now) == st).count();
            out.push(ScopeSummary {
                active: count(EntryState::Active),
                pending: count(EntryState::Pending),
                entries: entries.len(),
                scope,
            });
        }
        Ok(out)
    }
}
