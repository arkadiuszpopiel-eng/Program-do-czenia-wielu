//! Komendy `turns_*` dotyczące jednej tury: adnotacje (ocena, ukrycie) i intencje
//! (pamięć, czytanie na głos, zapis/uruchomienie kodu, cofnięcie kroku).

use memory_contract::{Memory, NewMemory, RememberMode};
use sessions_contract::{SessionHistory, SessionId};

use crate::core::AppCore;
use crate::dto::{BrokerIntentResult, Rating, RememberScope};
use crate::error::AppError;
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

    /// `turns_remember` ⟶ pamięć (v0: zakres sesji; projekt/globalna/agentka — F7).
    pub async fn turns_remember(
        &self,
        turn_id: String,
        scope: RememberScope,
    ) -> Result<(), AppError> {
        let (id, turn) = ids::parse_turn(&turn_id)?;
        if scope != RememberScope::Session {
            return Err(AppError::unavailable(
                "Pamięć projektu, globalna i agentki",
                "memory (zakresy F7)",
            ));
        }
        let turn = self.inner.sessions.turn(&id, turn)?;
        let text = turn.content.text.trim();
        if text.is_empty() {
            return Err(AppError::invalid("Ta tura nie ma treści do zapamiętania."));
        }
        self.inner
            .memory
            .remember(NewMemory::user_fact(id, text), RememberMode::Explicit)?;
        Ok(())
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
        self.inner
            .shell
            .save_text_as(&format!("kod.{ext}"), &code)?;
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

    /// `turns_undo_step` ⟶ dziennik cofania (`fs.*`, przez Brokera).
    pub async fn turns_undo_step(&self, undo_token: String) -> Result<(), AppError> {
        self.inner.broker.undo_step(&undo_token).await
    }
}
