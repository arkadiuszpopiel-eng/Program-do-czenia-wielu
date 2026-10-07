//! Cofanie kroków agentek spoza dziennika `undo-journal` (karta „Cofnij”): zapis schowka
//! (`tools-clipboard`) i zapis zmiennej użytkownika (`tools-system`, `system_env_set`). Krok należy
//! do sesji, której przebieg go utworzył; konflikt (wartość zmieniona później) — odmowa.

use app_agents::{AgentTools, ClipboardUndoError, EnvUndoError};
use app_api::ids::UndoKind;
use app_api::{AppError, ErrorCode};
use sessions_contract::SessionId;

use crate::engine::ChatEngine;

fn clipboard(tools: &AgentTools, id: u64) -> Result<String, AppError> {
    tools.undo_clipboard(id).map_err(|e| {
        let code = match e {
            ClipboardUndoError::Unknown(_) => ErrorCode::NotFound,
            ClipboardUndoError::Conflict => ErrorCode::Forbidden,
            ClipboardUndoError::Platform(_) => ErrorCode::Unavailable,
        };
        AppError::new(code, format!("Cofnięcie: {e}"))
    })?;
    Ok("przywrócono poprzednią zawartość schowka".into())
}

fn env(tools: &AgentTools, id: u64) -> Result<String, AppError> {
    tools.undo_env(id).map_err(|e| {
        let code = match e {
            EnvUndoError::Unknown(_) => ErrorCode::NotFound,
            EnvUndoError::Conflict(_) => ErrorCode::Forbidden,
            EnvUndoError::Platform(_) => ErrorCode::Unavailable,
        };
        AppError::new(code, format!("Cofnięcie: {e}"))
    })
}

impl ChatEngine {
    /// Cofa krok schowka albo zmiennej tej sesji; zwraca opis dla UI. Kroki dziennika
    /// (`UndoKind::Journal`) cofa Broker — tu odmowa.
    pub fn undo_owned(
        &self,
        session: &SessionId,
        kind: UndoKind,
        id: u64,
    ) -> Result<String, AppError> {
        let (what, module): (&str, &str) = match kind {
            UndoKind::Clipboard => ("Cofnięcie zapisu schowka", "tools-clipboard"),
            UndoKind::System => ("Cofnięcie zapisu zmiennej", "tools-system"),
            UndoKind::Journal => {
                return Err(AppError::invalid("Krok dziennika cofania cofa Broker."));
            }
        };
        if !self.owns_undo(session, kind, id) {
            return Err(AppError::not_found("Ten krok nie należy do tej sesji."));
        }
        let agents = self
            .inner
            .agents
            .as_ref()
            .ok_or_else(|| AppError::unavailable(what, module))?;
        let text = match kind {
            UndoKind::System => env(&agents.tools, id)?,
            _ => clipboard(&agents.tools, id)?,
        };
        self.release_undo(session, kind, id);
        Ok(text)
    }
}
