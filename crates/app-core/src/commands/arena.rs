//! Komendy Areny (P4): `turns_arena_answer` — odpowiedź modelu przypiętego w Routerze jako nowa
//! gałąź (jak „Ponów”, historia append-only), `turns_select_branch` — wybór gałęzi odpowiedzi.
//! Logika w `app-chat` (`arena.rs`); tu tylko identyfikatory, blokada sesji i delegacja.

use sessions_contract::SessionHistory;

use crate::core::AppCore;
use crate::error::AppError;
use crate::ids;
use app_chat::GenRequest;

impl AppCore {
    /// `turns_arena_answer`: odpowiedź modelu `dostawca:model` pod wiadomością użytkownika
    /// (albo obok odpowiedzi). Kończy trwającą generację sesji, jak `turns_regenerate`.
    pub async fn turns_arena_answer(
        &self,
        session_id: String,
        turn_id: String,
        model: String,
    ) -> Result<String, AppError> {
        let id = ids::session(&session_id)?;
        let target = ids::parse_turn_in(&id, &turn_id)?;
        let _guard = self.lock_session(&id).await;
        self.chat().finalize_generation(&id).await;
        let turn = self.inner.sessions.turn(&id, target)?;
        let (placement, leaf) = app_chat::arena_placement(&turn)?;
        let agent = self.agent_of(&id, &turn);
        self.chat()
            .start_pinned_generation(
                GenRequest {
                    session: id,
                    placement,
                    history_leaf: leaf,
                    agent,
                    profile: None,
                    continues: None,
                    origin: risk_classifier_contract::CommandOrigin::UserText,
                    tap: None,
                },
                &model,
            )
            .await
    }

    /// `turns_select_branch`: „Wybierz tę odpowiedź” — gałąź tury staje się aktywną ścieżką.
    pub async fn turns_select_branch(
        &self,
        session_id: String,
        turn_id: String,
    ) -> Result<(), AppError> {
        let id = ids::session(&session_id)?;
        let turn = ids::parse_turn_in(&id, &turn_id)?;
        let _guard = self.lock_session(&id).await;
        self.chat().select_branch(&id, turn)?;
        Ok(())
    }
}
