//! Reguły wspólne dla `-impl` i `-fake`: autoryzacja, fuzja RRF, deterministyczny porządek.

use std::collections::BTreeMap;

use crate::error::SearchError;
use crate::types::{Caller, Hit, SessionSet};

/// Górny limit liczby trafień w jednym zapytaniu.
pub const MAX_LIMIT: usize = 200;

/// Stała `k` w Reciprocal Rank Fusion (`1 / (k + pozycja)`).
pub const RRF_K: f32 = 60.0;

/// Agentka może pytać wyłącznie o własną sesję (`SessionSet::One(własna)`); właściciel o dowolne.
///
/// Wyszukiwanie między sesjami nigdy nie jest narzędziem agentki (PLAN §10, ADR 0008).
pub fn authorize(sessions: &SessionSet, caller: &Caller) -> Result<(), SearchError> {
    match (caller, sessions) {
        (Caller::Owner, _) => Ok(()),
        (Caller::Agent { session }, SessionSet::One(target)) if target == session => Ok(()),
        (Caller::Agent { .. }, _) => Err(SearchError::Forbidden {
            reason: "agentka przeszukuje wyłącznie własną sesję".into(),
        }),
    }
}

/// Reciprocal Rank Fusion list rankingowych (pozycja od 1): `Σ 1/(RRF_K + pozycja)`.
/// Wynik posortowany malejąco po wyniku, remisy rosnąco po kluczu (deterministycznie).
pub fn fuse_rrf<K: Ord + Clone>(lists: &[Vec<K>]) -> Vec<(K, f32)> {
    let mut scores: BTreeMap<K, f32> = BTreeMap::new();
    for list in lists {
        for (rank, key) in list.iter().enumerate() {
            let pos = f32::from(u16::try_from(rank + 1).unwrap_or(u16::MAX));
            *scores.entry(key.clone()).or_insert(0.0) += 1.0 / (RRF_K + pos);
        }
    }
    let mut out: Vec<(K, f32)> = scores.into_iter().collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

/// Deterministyczny porządek trafień: wynik malejąco, potem sesja i dokument rosnąco.
pub fn sort_hits(hits: &mut [Hit]) {
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.session.cmp(&b.session))
            .then_with(|| a.doc.cmp(&b.doc))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_bus_contract::SessionId;

    #[test]
    fn agent_limited_to_own_session() {
        let a = SessionId::new("A");
        let agent = Caller::Agent { session: a.clone() };
        assert!(authorize(&SessionSet::One(a.clone()), &agent).is_ok());
        assert!(authorize(&SessionSet::One(SessionId::new("B")), &agent).is_err());
        assert!(authorize(&SessionSet::All, &agent).is_err());
        assert!(authorize(&SessionSet::Many(vec![a]), &agent).is_err());
        assert!(authorize(&SessionSet::All, &Caller::Owner).is_ok());
    }

    #[test]
    fn rrf_prefers_items_in_both_lists() {
        let fused = fuse_rrf(&[vec!["a", "b"], vec!["b", "c"]]);
        let keys: Vec<&str> = fused.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec!["b", "a", "c"]);
        assert!(fused[0].1 > fused[1].1);
        // Remis → porządek kluczy.
        let tie = fuse_rrf(&[vec!["y"], vec!["x"]]);
        assert_eq!(tie[0].0, "x");
    }
}
