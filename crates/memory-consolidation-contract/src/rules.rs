//! Reguły deterministyczne konsolidacji (bez modelu): retencja, deduplikacja, sprzeczności
//! tematów, walidacja propozycji modelu → [`ChangeOp`].

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use memory_contract::rerank::content_stems;
use memory_contract::{
    AgentId, ChangeKind, ChangeOp, Derivation, EntryRef, EntryState, JournalRecord, Layer,
    MemoryEntry, MemoryId, NewMemory, Origin, Provenance, entry_state, normalized_text,
    subject_key, trust_rank,
};

use crate::config::{AutoExtract, ConsolidationConfig};
use crate::ports::{ConsolidationBatch, ConsolidatorOutput};

/// Agentka zapisana w proweniencji faktów z konsolidacji (rola Strażniczki pamięci).
pub const GUARDIAN_AGENT: &str = "strazniczka-pamieci";

/// Podobieństwo Jaccarda zbiorów rdzeni.
pub fn jaccard(a: &[String], b: &[String]) -> f32 {
    let union = a.iter().chain(b).collect::<BTreeSet<_>>().len();
    if union == 0 {
        return 0.0;
    }
    let inter = a.iter().filter(|x| b.contains(x)).count();
    let f = |n: usize| f32::from(u16::try_from(n).unwrap_or(u16::MAX));
    f(inter) / f(union)
}

/// Maksymalna długość treści propozycji modelu (znaki).
pub const MAX_PROPOSAL_CHARS: usize = 2000;
/// Limity propozycji na wsad: fakty, streszczenia, umiejętności.
pub const MAX_PROPOSALS: (usize, usize, usize) = (20, 5, 5);

/// Temat faktu: jawny albo z wzorca „X to Y” / „X: Y” / „X = Y” / „X wynosi Y” (1–6 słów tematu).
pub fn subject_of(entry: &MemoryEntry) -> Option<String> {
    if let Some(s) = entry
        .subject
        .as_deref()
        .map(subject_key)
        .filter(|s| !s.is_empty())
    {
        return Some(s);
    }
    let text = entry.text.trim().trim_end_matches('.');
    for sep in [" to ", ": ", " = ", " wynosi "] {
        if let Some((head, tail)) = text.split_once(sep) {
            let key = subject_key(head);
            let words = key.split(' ').filter(|w| !w.is_empty()).count();
            if (1..=6).contains(&words) && !normalized_text(tail).is_empty() {
                return Some(key);
            }
        }
    }
    None
}

fn active(e: &MemoryEntry, now: DateTime<Utc>) -> bool {
    entry_state(e, now) == EntryState::Active
}

/// Wpisy do wygaszenia: wygasłe (TTL) oraz przetworzone epizody starsze niż retencja
/// (bez przypiętych).
pub fn retention(
    entries: &[MemoryEntry],
    now: DateTime<Utc>,
    cfg: &ConsolidationConfig,
) -> Vec<ChangeOp> {
    let limit = cfg
        .episodic_retention_days
        .and_then(|d| i64::try_from(d).ok())
        .and_then(chrono::Duration::try_days);
    entries
        .iter()
        .filter(|e| !e.pinned)
        .filter(|e| {
            entry_state(e, now) == EntryState::Expired
                || (e.layer == Layer::Episodic
                    && e.consolidated_at.is_some()
                    && limit.is_some_and(|l| now - e.created_at > l))
        })
        .map(|e| ChangeOp::Expire {
            id: e.id.clone(),
            note: "retencja".into(),
        })
        .collect()
}

fn rank_key(
    e: &MemoryEntry,
) -> (
    bool,
    u8,
    u32,
    std::cmp::Reverse<DateTime<Utc>>,
    std::cmp::Reverse<MemoryId>,
) {
    let conf = (e.confidence.clamp(0.0, 1.0) * 1000.0).round();
    let conf = if conf.is_finite() { conf as u32 } else { 0 };
    (
        e.pinned,
        trust_rank(&e.provenance),
        conf,
        std::cmp::Reverse(e.created_at),
        std::cmp::Reverse(e.id.clone()),
    )
}

/// Duplikaty wśród aktywnych faktów i umiejętności (ta sama warstwa): identyczna treść
/// znormalizowana albo podobieństwo rdzeni ≥ progu przy zgodnym temacie. Wiodący: przypięty,
/// najwyższe zaufanie, pewność, najstarszy.
pub fn duplicates(
    entries: &[MemoryEntry],
    now: DateTime<Utc>,
    cfg: &ConsolidationConfig,
) -> Vec<ChangeOp> {
    let pool: Vec<&MemoryEntry> = entries
        .iter()
        .filter(|e| active(e, now) && matches!(e.layer, Layer::Semantic | Layer::Procedural))
        .collect();
    let stems: Vec<Vec<String>> = pool.iter().map(|e| content_stems(&e.text)).collect();
    let mut parent: Vec<usize> = (0..pool.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..pool.len() {
        for j in (i + 1)..pool.len() {
            let (a, b) = (pool[i], pool[j]);
            if a.layer != b.layer {
                continue;
            }
            let same = normalized_text(&a.text) == normalized_text(&b.text);
            let near = subject_of(a) == subject_of(b)
                && jaccard(&stems[i], &stems[j]) >= cfg.dedup_similarity;
            if same || near {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<&MemoryEntry>> = BTreeMap::new();
    for (i, entry) in pool.iter().enumerate() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(entry);
    }
    let mut ops = Vec::new();
    for group in groups.into_values().filter(|g| g.len() > 1) {
        let Some(keep) = group.iter().max_by_key(|e| rank_key(e)) else {
            continue;
        };
        let mut dups: Vec<MemoryId> = group
            .iter()
            .filter(|e| e.id != keep.id)
            .map(|e| e.id.clone())
            .collect();
        dups.sort();
        ops.push(ChangeOp::Merge {
            keep: keep.id.clone(),
            duplicates: dups,
            note: format!("scalono {} duplikatów", group.len() - 1),
        });
    }
    ops
}

/// Sprzeczności: aktywne fakty semantyczne o tym samym temacie i różnej treści. Najnowszy zastępuje
/// starsze, gdy jest zaufany i ma zaufanie ≥; inaczej konflikt (jeden raz — dziennik pamięta).
pub fn contradictions(
    entries: &[MemoryEntry],
    skip: &BTreeSet<MemoryId>,
    journal: &[JournalRecord],
    now: DateTime<Utc>,
) -> Vec<ChangeOp> {
    let mut by_subject: BTreeMap<String, Vec<&MemoryEntry>> = BTreeMap::new();
    for e in entries {
        if e.layer == Layer::Semantic
            && active(e, now)
            && !skip.contains(&e.id)
            && let Some(s) = subject_of(e)
        {
            by_subject.entry(s).or_default().push(e);
        }
    }
    let flagged = |a: &MemoryId, b: &MemoryId| {
        journal
            .iter()
            .any(|j| j.kind == ChangeKind::Conflict && j.touches(a) && j.touches(b))
    };
    let mut ops = Vec::new();
    for group in by_subject.into_values() {
        let Some(newest) = group.iter().max_by_key(|e| (e.created_at, e.id.clone())) else {
            continue;
        };
        let value = normalized_text(&newest.text);
        for old in group
            .iter()
            .filter(|e| e.id != newest.id && normalized_text(&e.text) != value)
        {
            if newest.trusted && trust_rank(&newest.provenance) >= trust_rank(&old.provenance) {
                ops.push(ChangeOp::Resolve {
                    old: old.id.clone(),
                    by: newest.id.clone(),
                    note: "sprzeczność tematu: nowszy fakt".into(),
                });
            } else if !flagged(&old.id, &newest.id) {
                ops.push(ChangeOp::FlagConflict {
                    a: old.id.clone(),
                    b: newest.id.clone(),
                    note: "sprzeczność do decyzji użytkownika".into(),
                });
            }
        }
    }
    ops
}

/// Zamienia propozycje modelu na zmiany: tylko źródła z wsadu, treść niepusta i ograniczona,
/// bez duplikatów znanych faktów; fakty zgodnie z `auto_extract`, umiejętności zawsze oczekujące;
/// na końcu oznaczenie epizodów jako przetworzonych. Zwraca też liczbę odrzuconych propozycji.
pub fn proposals_to_ops(
    batch: &ConsolidationBatch,
    output: &ConsolidatorOutput,
    existing: &[MemoryEntry],
    cfg: &ConsolidationConfig,
) -> (Vec<ChangeOp>, usize) {
    let ids: BTreeSet<&MemoryId> = batch.episodes.iter().map(|e| &e.id).collect();
    let mut known: BTreeSet<String> = existing.iter().map(|e| normalized_text(&e.text)).collect();
    let mut ops = Vec::new();
    let mut rejected = 0;
    let mut make = |text: &str,
                    sources: &[MemoryId],
                    layer: Layer,
                    derivation: Derivation|
     -> Option<NewMemory> {
        let norm = normalized_text(text);
        let ok = !norm.is_empty()
            && text.chars().count() <= MAX_PROPOSAL_CHARS
            && !sources.is_empty()
            && sources.iter().all(|s| ids.contains(s))
            && known.insert(norm);
        ok.then(|| NewMemory {
            origin: Origin::derived(
                derivation,
                sources
                    .iter()
                    .map(|s| EntryRef::new(batch.scope.clone(), s.clone()))
                    .collect(),
            ),
            ..NewMemory::new(
                batch.scope.clone(),
                layer,
                text.trim(),
                Provenance::Agent {
                    agent: AgentId::new(GUARDIAN_AGENT),
                },
            )
        })
    };
    let (max_f, max_s, max_k) = MAX_PROPOSALS;
    for f in output.facts.iter().take(max_f) {
        match make(&f.text, &f.sources, Layer::Semantic, Derivation::Extracted) {
            Some(mut new) => {
                new.subject = f
                    .subject
                    .clone()
                    .filter(|s| !s.trim().is_empty() && s.chars().count() <= 128);
                new.entities = f
                    .entities
                    .iter()
                    .filter(|e| !e.trim().is_empty() && e.chars().count() <= 128)
                    .take(32)
                    .cloned()
                    .collect();
                new.confidence = if f.confidence.is_finite() {
                    f.confidence.clamp(0.0, 1.0)
                } else {
                    0.5
                };
                let approved = cfg.auto_extract == AutoExtract::On;
                ops.push(ChangeOp::Create {
                    entry: new,
                    approved,
                    note: "fakt z epizodów".into(),
                });
            }
            None => rejected += 1,
        }
    }
    for s in output.summaries.iter().take(max_s) {
        match make(&s.text, &s.sources, Layer::Episodic, Derivation::Summary) {
            Some(new) => ops.push(ChangeOp::Create {
                entry: new,
                approved: true,
                note: "streszczenie".into(),
            }),
            None => rejected += 1,
        }
    }
    for k in output.skills.iter().take(max_k) {
        let text = format!("{}: {}", k.title.trim(), k.text.trim());
        match make(&text, &k.sources, Layer::Procedural, Derivation::Skill) {
            Some(new) => ops.push(ChangeOp::Create {
                entry: new,
                approved: false,
                note: "umiejętność".into(),
            }),
            None => rejected += 1,
        }
    }
    rejected += output.facts.len().saturating_sub(max_f)
        + output.summaries.len().saturating_sub(max_s)
        + output.skills.len().saturating_sub(max_k);
    ops.push(ChangeOp::MarkConsolidated {
        ids: batch.episodes.iter().map(|e| e.id.clone()).collect(),
    });
    (ops, rejected)
}
