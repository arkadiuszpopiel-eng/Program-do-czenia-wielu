//! Sesje przy imporcie: porównanie wersji lokalnej z paczką i scalanie drzew tur (suma zbiorów,
//! bez zmiany istniejących tur — nowe tury dostają kolejne identyfikatory i gałęzie wg reguł
//! `TreeCursor`, tak jakby dopisano je przez `append_turn`/`fork_from`).

use std::collections::BTreeMap;

use sessions_contract::{HeardPrefix, PortableSession, TreeCursor, Turn, TurnId};

use crate::error::TransferError;
use crate::report::ItemState;

/// Porównanie sesji lokalnej z wersją z paczki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDiff {
    /// Stan.
    pub state: ItemState,
    /// Czy paczka jest „kontynuacją” wersji lokalnej (fast-forward).
    pub fast_forward: bool,
    /// Opis dla dry-run.
    pub detail: String,
}

/// Klucz treści tury niezależny od numeracji (rola, autor, treść, koszt, czas).
fn content_key(t: &Turn) -> Vec<u8> {
    serde_json::to_vec(&(&t.role, &t.author, &t.content, &t.usage, &t.created_at))
        .unwrap_or_default()
}

/// Porównuje sesje (ten sam identyfikator).
pub fn diff_sessions(local: &PortableSession, package: &PortableSession) -> SessionDiff {
    let common = local.turns.len().min(package.turns.len());
    let prefix_equal = local.turns[..common]
        .iter()
        .zip(&package.turns[..common])
        .all(|(a, b)| a.fingerprint() == b.fingerprint());
    if !prefix_equal {
        return SessionDiff {
            state: ItemState::Collision,
            fast_forward: false,
            detail: "rozbieżna historia (ta sama sesja z dwóch maszyn)".to_owned(),
        };
    }
    let (l, p) = (local.turns.len(), package.turns.len());
    if p > l {
        return SessionDiff {
            state: ItemState::Changed,
            fast_forward: true,
            detail: format!("+{} tur z paczki", p - l),
        };
    }
    if l > p {
        return SessionDiff {
            state: ItemState::Changed,
            fast_forward: false,
            detail: format!("lokalnie więcej tur (+{})", l - p),
        };
    }
    if local == package {
        return SessionDiff {
            state: ItemState::Same,
            fast_forward: false,
            detail: "identyczna".to_owned(),
        };
    }
    SessionDiff {
        state: ItemState::Changed,
        fast_forward: false,
        detail: "różne metadane, prefiksy lub stan widoku".to_owned(),
    }
}

/// Wynik scalania drzew.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeResult {
    /// Nowe tury do dopisania (`import_turns`), z przemapowanymi identyfikatorami.
    pub new_turns: Vec<Turn>,
    /// Usłyszane prefiksy do dopisania do istniejących tur (fakt znany tylko w paczce).
    pub heard: Vec<(TurnId, HeardPrefix)>,
    /// Mapa `id w paczce → id lokalny`.
    pub map: BTreeMap<TurnId, TurnId>,
}

/// Znana tura pod rodzicem: `(id, klucz treści, czy ma usłyszany prefiks)`.
type Known = (TurnId, Vec<u8>, bool);

/// Scala drzewo z paczki z lokalnym: tura o tym samym rodzicu i tej samej treści (z czasem) to
/// ta sama tura; pozostałe są dopisywane jako nowe (gałęzie jak przy `fork_from`).
pub fn merge_turns(local: &[Turn], package: &[Turn]) -> Result<MergeResult, TransferError> {
    let mut cursor = TreeCursor::from_turns(local);
    let mut children: BTreeMap<Option<TurnId>, Vec<Known>> = BTreeMap::new();
    for t in local {
        children.entry(t.parent).or_default().push((
            t.id,
            content_key(t),
            t.heard_prefix.is_some(),
        ));
    }
    let mut out = MergeResult::default();
    for p in package {
        let parent = match p.parent {
            None => None,
            Some(pp) => Some(*out.map.get(&pp).ok_or_else(|| {
                TransferError::invalid("turns", format!("tura {} przed rodzicem {pp}", p.id))
            })?),
        };
        let key = content_key(p);
        let siblings = children.entry(parent).or_default();
        if let Some((id, _, has_heard)) = siblings.iter_mut().find(|(_, k, _)| *k == key) {
            if let (Some(prefix), false) = (p.heard_prefix, *has_heard) {
                out.heard.push((*id, prefix));
                *has_heard = true;
            }
            out.map.insert(p.id, *id);
            continue;
        }
        let (branch, _) = cursor
            .branch_for(parent)
            .map_err(|e| TransferError::invalid("turns", e))?;
        let mut turn = p.clone();
        turn.id = cursor.next_turn();
        turn.parent = parent;
        turn.branch = branch;
        cursor
            .accept(&turn)
            .map_err(|e| TransferError::invalid("turns", e))?;
        siblings.push((turn.id, key, turn.heard_prefix.is_some()));
        out.map.insert(p.id, turn.id);
        out.new_turns.push(turn);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use sessions_contract::{Author, BranchId, Role, TurnContent};

    use super::*;

    fn t(id: u64, parent: Option<u64>, branch: u64, text: &str, sec: i64) -> Turn {
        Turn {
            id: TurnId(id),
            parent: parent.map(TurnId),
            branch: BranchId(branch),
            role: Role::User,
            author: Author::User,
            content: TurnContent::text(text),
            usage: None,
            created_at: Utc
                .timestamp_opt(1_790_000_000 + sec, 0)
                .single()
                .unwrap_or_default(),
            heard_prefix: None,
            hidden: false,
        }
    }

    #[test]
    fn merge_is_union_and_keeps_local_turns() {
        // Wspólne: 1 → 2. Lokalnie dalej 3 (dziecko 2). W paczce inne dziecko 2 i jego dziecko.
        let local = vec![
            t(1, None, 1, "a", 1),
            t(2, Some(1), 1, "b", 2),
            t(3, Some(2), 1, "lok", 3),
        ];
        let mut pkg = vec![
            t(1, None, 1, "a", 1),
            t(2, Some(1), 1, "b", 2),
            t(3, Some(2), 1, "zdal", 4),
            t(4, Some(3), 1, "dalej", 5),
        ];
        pkg[1].heard_prefix = None;
        let m = merge_turns(&local, &pkg).unwrap();
        assert_eq!(m.new_turns.len(), 2);
        assert_eq!(m.new_turns[0].id, TurnId(4));
        assert_eq!(m.new_turns[0].parent, Some(TurnId(2)));
        assert_eq!(m.new_turns[0].branch, BranchId(2), "wariant → nowa gałąź");
        assert_eq!(m.new_turns[1].id, TurnId(5));
        assert_eq!(
            m.new_turns[1].branch,
            BranchId(2),
            "kontynuacja gałęzi wariantu"
        );
        assert_eq!(m.map[&TurnId(3)], TurnId(4));
        let mut all = local.clone();
        all.extend(m.new_turns.clone());
        assert!(TreeCursor::empty().check_batch(&all).is_ok());
        // Ponowne scalenie niczego nie dodaje (idempotentne).
        let again = merge_turns(&all, &pkg).unwrap();
        assert!(again.new_turns.is_empty());
    }

    #[test]
    fn diff_detects_fast_forward_and_collision() {
        let meta_src = |turns: Vec<Turn>| PortableSession {
            meta: sessions_contract::SessionMeta {
                id: sessions_contract::SessionId::new("s"),
                title: "t".into(),
                template: Default::default(),
                model_policy: "auto".into(),
                agents: vec![],
                privacy: Default::default(),
                tainted: false,
                workdir: "s".into(),
                pinned: false,
                archived: false,
                trashed: false,
                project: None,
                tags: vec![],
                created_at: Utc.timestamp_opt(0, 0).single().unwrap_or_default(),
                updated_at: Utc.timestamp_opt(0, 0).single().unwrap_or_default(),
            },
            turns,
            active_leaf: None,
            draft: None,
        };
        let base = vec![t(1, None, 1, "a", 1)];
        let longer = vec![t(1, None, 1, "a", 1), t(2, Some(1), 1, "b", 2)];
        let other = vec![t(1, None, 1, "x", 1)];
        assert_eq!(
            diff_sessions(&meta_src(base.clone()), &meta_src(base.clone())).state,
            ItemState::Same
        );
        let ff = diff_sessions(&meta_src(base.clone()), &meta_src(longer.clone()));
        assert_eq!((ff.state, ff.fast_forward), (ItemState::Changed, true));
        let back = diff_sessions(&meta_src(longer), &meta_src(base.clone()));
        assert_eq!((back.state, back.fast_forward), (ItemState::Changed, false));
        assert_eq!(
            diff_sessions(&meta_src(base), &meta_src(other)).state,
            ItemState::Collision
        );
    }
}
