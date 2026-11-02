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

/// Klucze argumentów będących ścieżką albo adresem — tylko w nich szukamy szablonów (treść pliku
/// może zawierać `{{imie}}` albo `<b>` naprawdę).
const PATH_KEYS: &[&str] = &[
    "path", "from", "to", "new_name", "root", "url", "cwd", "dir", "target", "source",
];

/// Szablon zamiast wartości: w argumencie ścieżki/adresu cały (`{{old_path}}`, `<url>`) albo
/// w środku (`zdjecia/{{ file_name }}`), w treści zapisu — cała treść (`<adresy_po_zmianie>`,
/// `{{wynik}}`). Lekki model przepisywał wzór zamiast podstawić wartość (pomiar F3 fs-20, fs-22,
/// próba „prices”, 1.11–2.11). Szablon w środku treści jest dozwolony (pliki szablonów, HTML).
pub(crate) fn placeholder(input: &serde_json::Value) -> Option<String> {
    let map = input.as_object()?;
    let word = |inner: &str| {
        let inner = inner.trim();
        !inner.is_empty()
            && inner.chars().count() <= 40
            && inner
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.'))
    };
    // `strict`: w treści `<…>` liczy się tylko z `_` albo spacją (nie `<br>`, `<html>`).
    let whole = |v: &str, strict: bool| {
        let mustache = v
            .strip_prefix("{{")
            .and_then(|v| v.strip_suffix("}}"))
            .is_some_and(word);
        let angle = v
            .strip_prefix('<')
            .and_then(|v| v.strip_suffix('>'))
            .is_some_and(|w| word(w) && (!strict || w.contains(['_', ' '])));
        mustache || angle
    };
    let inside = |v: &str| {
        v.split("{{")
            .skip(1)
            .any(|rest| rest.split_once("}}").is_some_and(|(w, _)| word(w)))
    };
    let path = PATH_KEYS.iter().find_map(|key| {
        let value = map.get(*key)?.as_str()?.trim();
        (whole(value, false) || inside(value)).then(|| format!("`{key}` = „{value}” to szablon, nie wartość — podstaw prawdziwą ścieżkę albo adres (np. z wyniku fs_list)"))
    });
    path.or_else(|| {
        let value = map.get("content")?.as_str()?.trim();
        whole(value, true).then(|| format!("`content` = „{value}” to szablon, nie treść — wpisz całą prawdziwą treść (np. na podstawie wyniku fs_read)"))
    })
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

#[cfg(test)]
mod tests {
    use super::placeholder;
    use serde_json::json;

    #[test]
    fn placeholders_in_paths_and_whole_content() {
        assert!(placeholder(&json!({"path": "{{old_path}}"})).is_some());
        assert!(placeholder(&json!({"url": "<url>"})).is_some());
        assert!(placeholder(&json!({"from": " {{ plik }} "})).is_some());
        assert!(placeholder(&json!({"path": "raport.txt"})).is_none());
        assert!(placeholder(&json!({"path": "zdjecia/{{ file_name }}"})).is_some());
        assert!(placeholder(&json!({"content": "<adresy_po_zmianie>"})).is_some());
        assert!(placeholder(&json!({"content": " {{wynik}} "})).is_some());
        assert!(placeholder(&json!({"content": "Panie {{imie}}"})).is_none());
        assert!(placeholder(&json!({"content": "<br>"})).is_none());
        assert!(placeholder(&json!({"path": "<a>/<b>"})).is_none());
        assert!(placeholder(&json!({"path": "a{{b"})).is_none());
        assert!(placeholder(&json!(["{{x}}"])).is_none());
    }
}
