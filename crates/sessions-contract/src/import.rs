//! Import historii (paczki `.alfa`, moduł `transfer`): walidacja drzewa tur z zachowanymi
//! identyfikatorami. Wspólna dla `-impl` i `-fake`, żeby import dawał dokładnie to drzewo, które
//! powstałoby z `append_turn`/`fork_from` w tej samej kolejności (append-only, bez zmian starych tur).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::SessionError;
use crate::ids::{BranchId, SessionId, TurnId};
use crate::session::SessionMeta;
use crate::turn::{Turn, validate_heard_prefix};

/// Maksymalna długość identyfikatora sesji przyjmowanego z zewnątrz (import, nazwa pliku bazy).
pub const MAX_PORTABLE_ID_LEN: usize = 64;

/// Czy identyfikator sesji jest bezpieczny jako nazwa pliku i katalogu (`[A-Za-z0-9_-]`, 1–64 znaki).
/// Import odrzuca inne — identyfikator trafia do ścieżki `<data_dir>\<id>.db` (ochrona przed
/// path traversal z obcej paczki).
pub fn is_portable_session_id(id: &SessionId) -> bool {
    let s = id.as_str();
    !s.is_empty()
        && s.len() <= MAX_PORTABLE_ID_LEN
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Sesja w postaci przenośnej (eksport/import `.alfa`): metadane, całe drzewo tur rosnąco po `id`,
/// aktywny liść i szkic composera. Nie zawiera klucza bazy (klucz jest per maszyna).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PortableSession {
    /// Metadane (z identyfikatorem i znacznikami czasu).
    pub meta: SessionMeta,
    /// Wszystkie tury (wszystkie gałęzie) rosnąco po `id`.
    pub turns: Vec<Turn>,
    /// Aktywny liść.
    pub active_leaf: Option<TurnId>,
    /// Szkic composera.
    pub draft: Option<String>,
}

impl PortableSession {
    /// Walidacja przed utworzeniem sesji: bezpieczny identyfikator, drzewo zgodne z regułami
    /// [`TreeCursor`], aktywny liść wśród tur. Wspólna dla `-impl` i `-fake`.
    pub fn validate(&self) -> Result<(), SessionError> {
        if !is_portable_session_id(&self.meta.id) {
            return Err(SessionError::invalid(format!(
                "niedozwolony identyfikator sesji `{}`",
                self.meta.id
            )));
        }
        TreeCursor::empty().check_batch(&self.turns)?;
        if let Some(leaf) = self.active_leaf
            && !self.turns.iter().any(|t| t.id == leaf)
        {
            return Err(SessionError::TurnNotFound { turn: leaf });
        }
        Ok(())
    }

    /// Czas ostatniej tury.
    pub fn last_turn_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.turns.iter().map(|t| t.created_at).max()
    }
}

/// Stan drzewa tur potrzebny do walidacji dopisywanych (importowanych) tur.
///
/// Reguły (te same, które stosują `append_turn` i `fork_from`):
/// - identyfikator tury = kolejny numer (`max + 1`), bez luk;
/// - tura bez rodzica (pierwsza albo alternatywny początek) zakłada **nową** gałąź;
/// - tura, której rodzic nie ma jeszcze dzieci, jest kontynuacją gałęzi rodzica;
/// - tura, której rodzic ma już dziecko, jest wariantem w **nowej** gałęzi (`max + 1`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeCursor {
    next_turn: u64,
    next_branch: u64,
    branch_of: BTreeMap<TurnId, BranchId>,
    has_children: BTreeSet<TurnId>,
}

impl TreeCursor {
    /// Kursor pustej sesji.
    pub fn empty() -> Self {
        Self {
            next_turn: 1,
            next_branch: 1,
            ..Self::default()
        }
    }

    /// Kursor z istniejących tur: `(id, rodzic, gałąź)` w dowolnej kolejności.
    pub fn from_entries(
        entries: impl IntoIterator<Item = (TurnId, Option<TurnId>, BranchId)>,
    ) -> Self {
        let mut cursor = Self::empty();
        for (id, parent, branch) in entries {
            cursor.next_turn = cursor.next_turn.max(id.0.saturating_add(1));
            cursor.next_branch = cursor.next_branch.max(branch.0.saturating_add(1));
            cursor.branch_of.insert(id, branch);
            if let Some(p) = parent {
                cursor.has_children.insert(p);
            }
        }
        cursor
    }

    /// Kursor z pełnych tur.
    pub fn from_turns<'a>(turns: impl IntoIterator<Item = &'a Turn>) -> Self {
        Self::from_entries(turns.into_iter().map(|t| (t.id, t.parent, t.branch)))
    }

    /// Identyfikator, który dostanie następna tura.
    pub fn next_turn(&self) -> TurnId {
        TurnId(self.next_turn)
    }

    /// Identyfikator następnej nowej gałęzi.
    pub fn next_branch(&self) -> BranchId {
        BranchId(self.next_branch)
    }

    /// Czy tura istnieje.
    pub fn contains(&self, turn: TurnId) -> bool {
        self.branch_of.contains_key(&turn)
    }

    /// Gałąź, którą dostałaby następna tura o rodzicu `parent`, i czy to nowa gałąź.
    pub fn branch_for(&self, parent: Option<TurnId>) -> Result<(BranchId, bool), SessionError> {
        match parent {
            None => Ok((self.next_branch(), true)),
            Some(p) => {
                let branch = *self
                    .branch_of
                    .get(&p)
                    .ok_or(SessionError::TurnNotFound { turn: p })?;
                if self.has_children.contains(&p) {
                    Ok((self.next_branch(), true))
                } else {
                    Ok((branch, false))
                }
            }
        }
    }

    /// Sprawdza turę względem reguł i przesuwa kursor; zwraca `true`, gdy tura zakłada nową gałąź.
    pub fn accept(&mut self, turn: &Turn) -> Result<bool, SessionError> {
        if turn.id != self.next_turn() {
            return Err(SessionError::invalid(format!(
                "tura {} poza kolejnością (oczekiwano {})",
                turn.id,
                self.next_turn()
            )));
        }
        if turn.content.is_empty() {
            return Err(SessionError::EmptyTurn);
        }
        if let Some(prefix) = turn.heard_prefix {
            validate_heard_prefix(turn.role, &turn.content, prefix)?;
        }
        let (branch, new_branch) = self.branch_for(turn.parent)?;
        if turn.branch != branch {
            return Err(SessionError::invalid(format!(
                "tura {} w gałęzi {} (oczekiwano {branch})",
                turn.id, turn.branch
            )));
        }
        self.next_turn += 1;
        if new_branch {
            self.next_branch += 1;
        }
        self.branch_of.insert(turn.id, branch);
        if let Some(p) = turn.parent {
            self.has_children.insert(p);
        }
        Ok(new_branch)
    }

    /// Sprawdza całą partię bez zmiany kursora (import jest „wszystko albo nic”).
    pub fn check_batch(&self, turns: &[Turn]) -> Result<Vec<bool>, SessionError> {
        let mut probe = self.clone();
        turns.iter().map(|t| probe.accept(t)).collect()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::turn::{Author, Role, TurnContent};

    fn turn(id: u64, parent: Option<u64>, branch: u64) -> Turn {
        Turn {
            id: TurnId(id),
            parent: parent.map(TurnId),
            branch: BranchId(branch),
            role: Role::User,
            author: Author::User,
            content: TurnContent::text("x"),
            usage: None,
            created_at: Utc::now(),
            heard_prefix: None,
            hidden: false,
        }
    }

    #[test]
    fn portable_ids() {
        assert!(is_portable_session_id(&SessionId::new("sess-0001")));
        assert!(is_portable_session_id(&SessionId::new(
            "01933b2e-8f4a-7c3d-9e2f-0123456789ab"
        )));
        for bad in [
            "",
            "../x",
            "a/b",
            "a\\b",
            "C:",
            "x.db",
            "ą",
            &"a".repeat(65),
        ] {
            assert!(!is_portable_session_id(&SessionId::new(bad)), "{bad}");
        }
    }

    #[test]
    fn cursor_follows_append_and_fork_rules() {
        let mut c = TreeCursor::empty();
        assert_eq!(c.accept(&turn(1, None, 1)), Ok(true));
        assert_eq!(c.accept(&turn(2, Some(1), 1)), Ok(false));
        // Wariant tury 2 (rodzic 1 ma już dziecko) → nowa gałąź 2.
        assert_eq!(c.accept(&turn(3, Some(1), 2)), Ok(true));
        // Kontynuacja liścia 2 → gałąź 1.
        assert_eq!(c.accept(&turn(4, Some(2), 1)), Ok(false));
        // Alternatywny początek → nowa gałąź 3.
        assert_eq!(c.accept(&turn(5, None, 3)), Ok(true));
        assert!(c.accept(&turn(7, Some(1), 4)).is_err(), "luka w numeracji");
        assert!(c.accept(&turn(6, Some(4), 2)).is_err(), "zła gałąź");
        assert!(c.accept(&turn(6, Some(9), 1)).is_err(), "brak rodzica");
        let rebuilt = TreeCursor::from_entries([
            (TurnId(1), None, BranchId(1)),
            (TurnId(2), Some(TurnId(1)), BranchId(1)),
            (TurnId(3), Some(TurnId(1)), BranchId(2)),
            (TurnId(4), Some(TurnId(2)), BranchId(1)),
            (TurnId(5), None, BranchId(3)),
        ]);
        assert_eq!(rebuilt, c);
    }

    #[test]
    fn batch_check_does_not_move_cursor() {
        let c = TreeCursor::empty();
        assert_eq!(
            c.check_batch(&[turn(1, None, 1), turn(2, Some(1), 1)]),
            Ok(vec![true, false])
        );
        assert_eq!(c.next_turn(), TurnId(1));
        assert!(c.check_batch(&[turn(1, None, 2)]).is_err());
    }
}
