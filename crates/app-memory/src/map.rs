//! Typy `memory-contract` / `memory-consolidation-contract` → DTO Inspektora (bez treści innych
//! wpisów tam, gdzie kontrakt jej nie podaje).

use app_api::dto::{
    ConsolidationReport, MemoryExplanation, MemoryForgetReport, MemoryItem, MemoryJournalEntry,
    MemoryLayer, MemorySourceKind, MemorySourceLink, MemoryState, iso,
};
use memory_consolidation_contract::{RunReport, Trigger};
use memory_contract::{
    CascadeReport, ChangeKind, Derivation, EntryRef, EntryState, Explanation, InspectorItem,
    JournalRecord, Layer, MemoryEntry, Provenance, entry_state, expires_at, scope_key,
};

use crate::ids::{entry_dto, scope_ref};

/// Warstwa → DTO.
pub fn layer(l: Layer) -> MemoryLayer {
    match l {
        Layer::Working => MemoryLayer::Working,
        Layer::Episodic => MemoryLayer::Episodic,
        Layer::Semantic => MemoryLayer::Semantic,
        Layer::Procedural => MemoryLayer::Procedural,
    }
}

/// DTO → warstwa.
pub fn layer_of(l: MemoryLayer) -> Layer {
    match l {
        MemoryLayer::Working => Layer::Working,
        MemoryLayer::Episodic => Layer::Episodic,
        MemoryLayer::Semantic => Layer::Semantic,
        MemoryLayer::Procedural => Layer::Procedural,
    }
}

/// Stan → DTO.
pub fn state(s: EntryState) -> MemoryState {
    match s {
        EntryState::Active => MemoryState::Active,
        EntryState::Pending => MemoryState::Pending,
        EntryState::Superseded => MemoryState::Superseded,
        EntryState::Expired => MemoryState::Expired,
    }
}

/// DTO → stan.
pub fn state_of(s: MemoryState) -> EntryState {
    match s {
        MemoryState::Active => EntryState::Active,
        MemoryState::Pending => EntryState::Pending,
        MemoryState::Superseded => EntryState::Superseded,
        MemoryState::Expired => EntryState::Expired,
    }
}

fn derivation(d: Derivation) -> &'static str {
    match d {
        Derivation::Extracted => "extracted",
        Derivation::Summary => "summary",
        Derivation::Skill => "skill",
        Derivation::Promoted => "promoted",
        Derivation::Edited => "edited",
        Derivation::Imported => "imported",
    }
}

/// Wpis → pozycja Inspektora.
pub fn item(
    entry: &MemoryEntry,
    now: chrono::DateTime<chrono::Utc>,
    score: Option<f32>,
) -> MemoryItem {
    let (source, source_detail) = match &entry.provenance {
        Provenance::User => (MemorySourceKind::User, None),
        Provenance::Agent { agent } => (MemorySourceKind::Agent, Some(agent.to_string())),
        Provenance::UntrustedContent { source } => {
            (MemorySourceKind::Untrusted, Some(source.clone()))
        }
        Provenance::Import { source } => (MemorySourceKind::Import, Some(source.clone())),
    };
    MemoryItem {
        id: entry_dto(&entry.entry_ref()),
        scope: scope_ref(&entry.scope),
        scope_key: scope_key(&entry.scope),
        layer: layer(entry.layer),
        state: state(entry_state(entry, now)),
        text: entry.text.clone(),
        subject: entry.subject.clone(),
        entities: entry.entities.clone(),
        source,
        source_detail,
        trusted: entry.trusted,
        confidence: f64::from(entry.confidence),
        pinned: entry.pinned,
        version: entry.version,
        created_at: iso(entry.created_at),
        expires_at: expires_at(entry).map(iso),
        session_id: entry.origin.session.as_ref().map(ToString::to_string),
        turn: entry.origin.turn,
        derivation: entry.origin.derivation.map(|d| derivation(d).to_owned()),
        score: score.map(f64::from),
    }
}

/// Pozycja strony Inspektora.
pub fn inspector_item(i: &InspectorItem, now: chrono::DateTime<chrono::Utc>) -> MemoryItem {
    item(&i.entry, now, i.score)
}

fn change_kind(k: ChangeKind) -> &'static str {
    match k {
        ChangeKind::Create => "create",
        ChangeKind::Supersede => "supersede",
        ChangeKind::Merge => "merge",
        ChangeKind::Expire => "expire",
        ChangeKind::MarkConsolidated => "mark_consolidated",
        ChangeKind::Conflict => "conflict",
        ChangeKind::Edit => "edit",
        ChangeKind::Promote => "promote",
    }
}

/// Rekord dziennika → DTO.
pub fn journal(r: &JournalRecord) -> MemoryJournalEntry {
    MemoryJournalEntry {
        id: r.id.to_string(),
        scope_key: scope_key(&r.scope),
        run: r.run.clone(),
        at: iso(r.at),
        kind: change_kind(r.kind).to_owned(),
        note: r.note.clone(),
        entries: r
            .refs
            .iter()
            .map(|id| entry_dto(&EntryRef::new(r.scope.clone(), id.clone())))
            .collect(),
        undone: r.undone,
        undoable: !r.undone && !matches!(r.kind, ChangeKind::Expire | ChangeKind::Conflict),
    }
}

/// „Dlaczego to pamiętam" → DTO.
pub fn explanation(e: &Explanation, now: chrono::DateTime<chrono::Utc>) -> MemoryExplanation {
    MemoryExplanation {
        item: item(&e.entry, now, None),
        reasons: e.reasons.clone(),
        sources: e
            .sources
            .iter()
            .map(|s| MemorySourceLink {
                id: entry_dto(&s.entry),
                exists: s.exists,
                state: s.state.map(state),
            })
            .collect(),
        versions: e.versions.iter().map(|v| item(v, now, None)).collect(),
        merged: e.merged.iter().map(entry_dto).collect(),
        derived: e.derived.iter().map(entry_dto).collect(),
        journal: e.journal.iter().map(journal).collect(),
    }
}

fn n(x: usize) -> u64 {
    u64::try_from(x).unwrap_or(u64::MAX)
}

/// Raport kaskady → DTO.
pub fn forget_report(r: &CascadeReport) -> MemoryForgetReport {
    MemoryForgetReport {
        removed: n(r.removed.len()),
        derived: n(r.derived.len()),
        versions: n(r.versions.len()),
        revived: n(r.revived.len()),
        fts_rows: n(r.fts_rows),
        vectors: n(r.vectors),
        journal_records: n(r.journal_records),
        shredded: r.shredded.iter().map(scope_key).collect(),
        stale_exports: r.stale_exports.clone(),
    }
}

/// Raport porządkowania → DTO.
pub fn run_report(r: &RunReport) -> ConsolidationReport {
    let sum =
        |f: fn(&memory_consolidation_contract::ScopeRun) -> usize| n(r.scopes.iter().map(f).sum());
    ConsolidationReport {
        run: r.run.clone(),
        manual: r.trigger == Trigger::Manual,
        started_at: iso(r.started_at),
        skipped: r.skipped.map(|s| s.describe().to_owned()),
        interrupted: r.interrupted.map(|s| s.describe().to_owned()),
        scopes: n(r.scopes.len()),
        created: sum(|s| s.created),
        merged: sum(|s| s.merged),
        resolved: sum(|s| s.resolved),
        expired: sum(|s| s.expired),
        conflicts: sum(|s| s.conflicts),
        proposals: n(r.proposals),
        llm_calls: n(r.llm_calls),
        budget_denied: r.budget_denied,
        errors: r.errors.clone(),
    }
}
