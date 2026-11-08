//! Komendy Tablicy (`board_*`): przypięte odpowiedzi; plik i reguły: `app_modules::board`.

use app_modules::board::{self, Pin};
use sessions_contract::{Author, SessionCatalog, SessionHistory};

use crate::core::AppCore;
use crate::dto::BoardItem;
use crate::error::AppError;
use crate::ids;

impl AppCore {
    fn board_file(&self) -> std::path::PathBuf {
        self.inner.paths.state().join(board::FILE)
    }

    /// `board_list`: przypięte odpowiedzi, najnowsze pierwsze.
    pub async fn board_list(&self) -> Result<Vec<BoardItem>, AppError> {
        Ok(board::view(&board::load(&self.board_file())))
    }

    /// `board_pin`: przypina odpowiedź agentki (kopia treści; rozmowa się nie zmienia).
    pub async fn board_pin(
        &self,
        session_id: String,
        turn_id: String,
    ) -> Result<Vec<BoardItem>, AppError> {
        let sid = ids::session(&session_id)?;
        let tid = ids::parse_turn_in(&sid, &turn_id)?;
        let turn = self.inner.sessions.turn(&sid, tid)?;
        let Author::Agent { agent } = &turn.author else {
            return Err(AppError::invalid("Przypiąć można tylko odpowiedź agentki."));
        };
        let now = chrono::Utc::now();
        let pin = Pin {
            id: format!("pin-{}", now.timestamp_micros()),
            session_title: self.inner.sessions.session(&sid)?.title,
            session_id,
            turn_id,
            agent: agent.to_string(),
            text: turn.content.text.clone(),
            pinned_at: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        };
        board::pin(&self.board_file(), pin)
    }

    /// `board_unpin`: odpina.
    pub async fn board_unpin(&self, id: String) -> Result<Vec<BoardItem>, AppError> {
        board::unpin(&self.board_file(), &id)
    }
}
