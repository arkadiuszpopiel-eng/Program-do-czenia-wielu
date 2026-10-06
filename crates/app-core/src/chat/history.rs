//! Historia append-only: dopisywanie dziecka dowolnej tury (gałąź, gdy rodzic ma już dzieci)
//! i projekcja gałęzi do wiadomości dla dostawcy.

use providers_contract::{ContentBlock, Message, Role as MsgRole};
use sessions_contract::{NewTurn, Role, SessionHistory, SessionId, Turn, TurnId};

use crate::core::AppCore;
use crate::dto::TurnStatus;
use crate::error::AppError;

/// Instrukcja dla „Kontynuuj" (bez prefill — dopisana jako wiadomość użytkownika).
pub const CONTINUE_HINT: &str =
    "Kontynuuj dokładnie od miejsca, w którym przerwałaś. Nie powtarzaj wcześniejszego tekstu.";

impl AppCore {
    /// Dopisuje turę jako dziecko `parent` (`None` = aktywny liść albo pierwsza tura).
    /// Rodzic z dziećmi → nowa gałąź (`fork_from` rodzeństwa) — historia się nie zmienia.
    pub(crate) fn append_child(
        &self,
        session: &SessionId,
        parent: Option<TurnId>,
        turn: NewTurn,
    ) -> Result<Turn, AppError> {
        let history = &self.inner.sessions;
        let parent = match parent {
            Some(p) => Some(p),
            None => history.active_leaf(session)?,
        };
        let Some(parent) = parent else {
            return Ok(history.append_turn(session, None, turn)?);
        };
        let leaf = history.latest_leaf(session, parent)?;
        if leaf == parent {
            return Ok(history.append_turn(session, Some(parent), turn)?);
        }
        let path = history.branch_projection(session, leaf)?;
        let child = path
            .iter()
            .skip_while(|t| t.id != parent)
            .nth(1)
            .ok_or_else(|| AppError::internal("brak dziecka tury w projekcji gałęzi"))?;
        Ok(history.fork_from(session, child.id, turn)?)
    }

    /// Wiadomości dla dostawcy: gałąź od korzenia do `leaf` (bez tur-błędów i komunikatów
    /// systemowych), sąsiednie wiadomości tej samej roli scalone, zaczyna się od użytkownika.
    pub(crate) fn branch_messages(
        &self,
        session: &SessionId,
        leaf: TurnId,
        continue_hint: bool,
    ) -> Result<Vec<Message>, AppError> {
        let path = self.inner.sessions.branch_projection(session, leaf)?;
        let metas = self.inner.store.metas(session)?;
        let mut out: Vec<Message> = Vec::new();
        for turn in &path {
            let failed = metas
                .get(&turn.id.0)
                .is_some_and(|m| m.status == Some(TurnStatus::Error));
            let role = match turn.role {
                Role::User => MsgRole::User,
                Role::Assistant if !failed => MsgRole::Assistant,
                _ => continue,
            };
            let text = turn.content.text.trim();
            let mut blocks = (self.inner.work.files).provider_blocks(session, &turn.content.blocks);
            if !text.is_empty() {
                blocks.insert(0, ContentBlock::text(text));
            }
            if blocks.is_empty() || (out.is_empty() && role == MsgRole::Assistant) {
                continue;
            }
            match out.last_mut() {
                Some(last) if last.role == role => last.content.extend(blocks),
                _ => out.push(Message::new(role, blocks)),
            }
        }
        if continue_hint {
            out.push(Message::user_text(CONTINUE_HINT));
        }
        if out.last().is_none_or(|m| m.role != MsgRole::User) {
            return Err(AppError::invalid(
                "Brak wiadomości użytkownika, na którą można odpowiedzieć.",
            ));
        }
        Ok(out)
    }
}
