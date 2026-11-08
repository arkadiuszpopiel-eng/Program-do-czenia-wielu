//! Arena (P4): ta sama wiadomość do dwóch modeli obok siebie. Każda odpowiedź to zwykła tura
//! agentki dopisana jako nowa gałąź pod wiadomością użytkownika (historia append-only — żadna
//! tura nie jest zmieniana), z modelem przypiętym w Routerze (`dostawca:model`). Budżet,
//! prywatność sesji, zgodność i Broker (skażenie sesji) działają jak w zwykłej turze.
//! „Wybierz tę odpowiedź" ustawia aktywny liść sesji na gałąź wybranej odpowiedzi.

use router_contract::Candidate;
use sessions_contract::{Role, SessionHistory, SessionId, Turn, TurnId};

use app_api::AppError;

use crate::engine::ChatEngine;
use crate::generate::{GenRequest, Placement};

/// Liczba modeli porównywanych w arenie.
pub const ARENA_MODELS: usize = 2;

/// Najdłuższy zapis `dostawca:model` przyjmowany z interfejsu.
const MODEL_MAX: usize = 200;

/// Model areny w zapisie `dostawca:model` (przypięcie Routera); `auto` i zapis bez dostawcy —
/// błąd (arena porównuje konkretne modele).
pub fn arena_model(raw: &str) -> Result<String, AppError> {
    let text = raw.trim();
    if text.len() > MODEL_MAX {
        return Err(AppError::invalid("Nazwa modelu jest za długa."));
    }
    Candidate::parse(text)
        .map(|c| c.qualified())
        .ok_or_else(|| {
            AppError::invalid(format!(
                "„{text}” nie jest modelem w zapisie dostawca:model (Arena porównuje konkretne modele)."
            ))
        })
}

/// Miejsce odpowiedzi areny i ostatnia tura historii dla modelu: wiadomość użytkownika → nowe
/// dziecko (gałąź obok wcześniejszych odpowiedzi); odpowiedź agentki → jej rodzeństwo.
pub fn arena_placement(turn: &Turn) -> Result<(Placement, TurnId), AppError> {
    match (turn.role, turn.parent) {
        (Role::User, _) => Ok((Placement::Child(turn.id), turn.id)),
        (_, Some(parent)) => Ok((Placement::Sibling(turn.id), parent)),
        (_, None) => Err(AppError::invalid("Tura bez wiadomości użytkownika.")),
    }
}

/// Wybór gałęzi: aktywny liść = najnowszy liść pod `turn` (dalsza rozmowa idzie tą gałęzią).
pub(crate) fn select_branch_in(
    history: &dyn SessionHistory,
    session: &SessionId,
    turn: TurnId,
) -> Result<TurnId, AppError> {
    history.turn(session, turn)?;
    let leaf = history.latest_leaf(session, turn)?;
    history.set_active_leaf(session, leaf)?;
    Ok(leaf)
}

impl ChatEngine {
    /// Odpowiedź areny: generacja z modelem przypiętym w Routerze. Wywołujący (jak przy „Ponów")
    /// trzyma blokadę sesji i zakończył poprzednią generację; drugą odpowiedź areny uruchamia
    /// interfejs po zakończeniu pierwszej (najwyżej jedna generacja na sesję).
    pub async fn start_pinned_generation(
        &self,
        req: GenRequest,
        model: &str,
    ) -> Result<String, AppError> {
        let pin = arena_model(model)?;
        self.start(req, Some(pin)).await
    }

    /// „Wybierz tę odpowiedź": gałąź tury staje się aktywną ścieżką sesji. Zwraca liść.
    pub fn select_branch(&self, session: &SessionId, turn: TurnId) -> Result<TurnId, AppError> {
        select_branch_in(&*self.inner.sessions, session, turn)
    }
}

#[cfg(test)]
#[path = "arena_tests.rs"]
mod tests;
