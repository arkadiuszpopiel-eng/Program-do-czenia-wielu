//! Komendy `turns_*` dotyczące jednej tury: adnotacje (ocena, ukrycie) i intencje
//! (pamięć, czytanie na głos, zapis/uruchomienie kodu, cofnięcie kroku).

use app_agents::ClipboardUndoError;
use sessions_contract::{SessionHistory, SessionId};

use crate::core::AppCore;
use crate::dto::{BrokerIntentResult, EventLevel, Rating, RememberScope, TimelineKind};
use crate::error::{AppError, ErrorCode};
use crate::ids;

impl AppCore {
    /// `turns_rate`: adnotacja (nie zmienia tury).
    pub async fn turns_rate(
        &self,
        turn_id: String,
        rating: Option<Rating>,
    ) -> Result<(), AppError> {
        let (id, turn) = ids::parse_turn(&turn_id)?;
        self.inner.sessions.turn(&id, turn)?;
        self.inner.store.push_rating(&id, turn, rating)
    }

    /// `turns_set_hidden`: „ukryj z widoku" (treść i audyt zostają).
    pub async fn turns_set_hidden(&self, turn_id: String, hidden: bool) -> Result<(), AppError> {
        let (id, turn) = ids::parse_turn(&turn_id)?;
        Ok(self.inner.sessions.set_hidden(&id, turn, hidden)?)
    }

    /// `turns_remember` ⟶ pamięć F7: sesja, projekt sesji, globalna albo agentka tury (jako
    /// właściciel — od razu aktywny; sesja prywatna nie zasila zakresów szerszych).
    pub async fn turns_remember(
        &self,
        turn_id: String,
        scope: RememberScope,
    ) -> Result<(), AppError> {
        let (id, number) = ids::parse_turn(&turn_id)?;
        let turn = self.inner.sessions.turn(&id, number)?;
        let text = turn.content.text.trim();
        if text.is_empty() {
            return Err(AppError::invalid("Ta tura nie ma treści do zapamiętania."));
        }
        let agent = self.agent_of(&id, &turn);
        self.inner
            .memory
            .remember_turn(&id, number.0, text, scope, &agent)
    }

    /// `turns_read_aloud` ⟶ TTS głosem agentki.
    pub async fn turns_read_aloud(&self, turn_id: String) -> Result<(), AppError> {
        let (id, turn) = ids::parse_turn(&turn_id)?;
        let turn = self.inner.sessions.turn(&id, turn)?;
        let agent = self.agent_of(&id, &turn);
        let spoken = lib_markdown::to_spoken_text(&turn.content.text);
        self.inner.voice.read_aloud(&agent, &spoken).await
    }

    fn code_block(
        &self,
        turn_id: &str,
        block: u64,
    ) -> Result<(SessionId, Option<String>, String), AppError> {
        let (id, turn) = ids::parse_turn(turn_id)?;
        let turn = self.inner.sessions.turn(&id, turn)?;
        let blocks =
            lib_markdown::render_blocks(&turn.content.text, lib_markdown::RenderOptions::default());
        let code = blocks
            .iter()
            .find(|b| b.id == block)
            .and_then(|b| b.code.first())
            .ok_or_else(|| AppError::not_found("Ten blok nie zawiera kodu."))?;
        Ok((id, code.lang.clone(), code.text.clone()))
    }

    /// `turns_save_code` ⟶ natywny dialog zapisu.
    pub async fn turns_save_code(&self, turn_id: String, block_index: u64) -> Result<(), AppError> {
        let (_, lang, code) = self.code_block(&turn_id, block_index)?;
        let ext = lang
            .as_deref()
            .filter(|l| l.chars().all(|c| c.is_ascii_alphanumeric()) && !l.is_empty())
            .unwrap_or("txt");
        // Natywny dialog blokuje wątek — poza pulą zadań async.
        let shell = self.inner.shell.clone();
        let name = format!("kod.{ext}");
        tokio::task::spawn_blocking(move || shell.save_text_as(&name, &code))
            .await
            .map_err(|e| AppError::internal(format!("okno zapisu: {e}")))??;
        Ok(())
    }

    /// `turns_run_code` ⟶ zawsze przez Brokera.
    pub async fn turns_run_code(
        &self,
        turn_id: String,
        block_index: u64,
    ) -> Result<BrokerIntentResult, AppError> {
        let (id, lang, code) = self.code_block(&turn_id, block_index)?;
        self.inner
            .broker
            .run_code(&id, lang.as_deref(), &code)
            .await
    }

    /// `turns_undo_step` ⟶ dziennik cofania (`fs.*`, snapshot powłoki: token `"<sesja>:u<krok>"`)
    /// albo zapis schowka (`"<sesja>:c<id>"`) — tylko kroki tej sesji.
    pub async fn turns_undo_step(&self, undo_token: String) -> Result<(), AppError> {
        let (session, kind, step) = ids::parse_any_undo(&undo_token)?;
        self.ensure_session(&session)?;
        let result = match kind {
            ids::UndoKind::Journal => self.inner.broker.undo_step(&session, step).await,
            ids::UndoKind::Clipboard => self.undo_clipboard(&session, step),
        };
        let (level, title) = match &result {
            Ok(text) => (EventLevel::Audit, format!("Cofnięto: {text}")),
            Err(e) => (
                EventLevel::Warn,
                format!("Cofnięcie nieudane: {}", e.message),
            ),
        };
        if result.is_ok()
            && let Err(e) = self.inner.store.push_undone(&session, &undo_token)
        {
            tracing::warn!(error = %e, "zapis cofniętego kroku nie powiódł się");
        }
        self.timeline_note(&session, TimelineKind::Tool, level, title, Some(undo_token));
        result.map(|_| ())
    }

    /// Cofnięcie zapisu schowka agentki (wykrywa konflikt: schowek zmieniony później).
    fn undo_clipboard(&self, session: &SessionId, id: u64) -> Result<String, AppError> {
        let owned = self
            .rt()
            .clip_undo
            .get(session)
            .is_some_and(|ids| ids.contains(&id));
        if !owned {
            return Err(AppError::not_found("Ten krok nie należy do tej sesji."));
        }
        let agents =
            self.inner.agents.as_ref().ok_or_else(|| {
                AppError::unavailable("Cofnięcie zapisu schowka", "tools-clipboard")
            })?;
        agents.tools.undo_clipboard(id).map_err(|e| {
            let code = match e {
                ClipboardUndoError::Unknown(_) => ErrorCode::NotFound,
                ClipboardUndoError::Conflict => ErrorCode::Forbidden,
                ClipboardUndoError::Platform(_) => ErrorCode::Unavailable,
            };
            AppError::new(code, format!("Cofnięcie: {e}"))
        })?;
        if let Some(ids) = self.rt().clip_undo.get_mut(session) {
            ids.remove(&id);
        }
        Ok("przywrócono poprzednią zawartość schowka".into())
    }
}
