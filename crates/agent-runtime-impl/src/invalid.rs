//! Niepoprawne argumenty wywołań narzędzi (C2.3). Dostawca zgłasza je jako
//! `ToolArguments::Invalid` (ucięty albo zepsuty JSON, u modelu lokalnego także argumenty
//! niezgodne ze schematem narzędzia), a akumulator tury zapisuje w historii `{}`. Takiego
//! wywołania nie wykonujemy — narzędzie z pustymi argumentami mogłoby zadziałać na wartościach
//! domyślnych. Krok kończy się błędem w osi czasu, a model dostaje wynik z opisem i prośbą
//! o ponowienie z poprawnym JSON-em (poprawka w następnej turze).

use std::collections::BTreeMap;

use agent_runtime_contract::{RunEvent, StepKind, StepStatus};
use providers_contract::{ProviderEvent, ToolArguments, ToolUse};
use tools_common_contract::text;

use crate::engine::Engine;
use crate::prompt::summary;

/// Najdłuższy opis błędu argumentów w wyniku dla modelu (znaki).
const MAX_ERROR_CHARS: usize = 300;

/// Zapamiętuje wywołania z niepoprawnymi argumentami (identyfikator → opis błędu). Po zmianie
/// modelu w trakcie tury wcześniejsze wywołania odpadają razem z blokami tury.
pub(crate) fn note(event: &ProviderEvent, invalid: &mut BTreeMap<String, String>) {
    match event {
        ProviderEvent::ToolCallEnd {
            id,
            arguments: ToolArguments::Invalid { error, .. },
            ..
        } => {
            invalid.insert(id.clone(), error.clone());
        }
        ProviderEvent::ModelSwitched { .. } => invalid.clear(),
        _ => {}
    }
}

/// Opis dla modelu (wynik „Nie wykonano: …”).
pub(crate) fn message(tool: &str, error: &str) -> String {
    let (error, _) = text::truncate_chars(error.trim(), MAX_ERROR_CHARS);
    format!(
        "argumenty wywołania `{tool}` są niepoprawne ({error}); narzędzie nie zostało \
         uruchomione — wywołaj je ponownie z jednym obiektem JSON zgodnym ze schematem parametrów"
    )
}

impl Engine {
    /// Wywołanie z niepoprawnymi argumentami: krok z błędem w osi czasu i opis dla modelu;
    /// `None` — argumenty poprawne.
    pub(crate) async fn reject_invalid(&mut self, tu: &ToolUse) -> Option<String> {
        let error = self.invalid_args.remove(&tu.id)?;
        let why = message(&tu.name, &error);
        let (step, input) = self.begin_call(tu).await;
        self.handle
            .emit(RunEvent::StepFinished {
                step,
                kind: StepKind::Tool,
                tool: Some(tu.name.clone()),
                status: StepStatus::Failed,
                input,
                output: summary(&why, 300),
                untrusted: false,
                undo: None,
                intent: None,
                approval: None,
            })
            .await;
        Some(why)
    }
}
