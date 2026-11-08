//! Renderowanie przerwanej tury („usłyszany prefiks", PLAN §6.5, ADR 0006).
//!
//! Historia jest append-only: przerwana tura asystentki zostaje **w pełnej postaci** (bloki myślenia
//! pozostają ważne), a informacja o tym, co użytkownik usłyszał, trafia do **nowej** wiadomości
//! tuż po niej. Dostawcy z natywnym obcięciem (OpenAI Realtime `conversation.item.truncate`)
//! obcinają po swojej stronie — projekcja nie dodaje wtedy notki.

use std::borrow::Cow;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::message::{ContentBlock, Message, Role};

/// Strategia renderowania przerwania (per adapter, `ProviderCapabilities::interruption`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterruptionRendering {
    /// Pełna tura + notka „Użytkownik usłyszał tylko: „…" i przerwał" jako nowa wiadomość.
    #[default]
    AppendNote,
    /// Obcięcie po stronie dostawcy; projekcja zostawia historię bez notek.
    NativeTruncate,
}

/// Treść notki o przerwaniu.
///
/// ```
/// use providers_contract::interruption_note;
/// assert_eq!(
///     interruption_note("Jutro będzie", false),
///     "Użytkownik usłyszał tylko: „Jutro będzie” i przerwał."
/// );
/// ```
pub fn interruption_note(heard: &str, approximate: bool) -> String {
    let heard = heard.trim();
    if heard.is_empty() {
        return "Użytkownik przerwał, zanim usłyszał jakąkolwiek część tej odpowiedzi.".into();
    }
    if approximate {
        format!("Użytkownik usłyszał tylko (w przybliżeniu): „{heard}” i przerwał.")
    } else {
        format!("Użytkownik usłyszał tylko: „{heard}” i przerwał.")
    }
}

fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Renderuje przerwaną turę: `[pełna tura bez zmian, notka]`. Gdy użytkownik usłyszał całość
/// (prefiks = cały widoczny tekst), notki nie ma. Oryginalna tura **nigdy** nie jest modyfikowana.
///
/// ```
/// use providers_contract::{render_interrupted_turn, Message, Role};
/// let full = Message::assistant_text("Jutro będzie słonecznie, ale wieczorem spadnie deszcz.");
/// let out = render_interrupted_turn(&full, "Jutro będzie słonecznie", false);
/// assert_eq!(out.len(), 2);
/// assert_eq!(out[0].as_ref(), &full);           // tura bajt w bajt
/// assert_eq!(out[1].role, Role::User);          // notka jako nowa wiadomość
/// assert!(out[1].visible_text().contains("„Jutro będzie słonecznie”"));
/// ```
pub fn render_interrupted_turn<'a>(
    full: &'a Message,
    heard: &str,
    approximate: bool,
) -> Vec<Cow<'a, Message>> {
    let mut out = vec![Cow::Borrowed(full)];
    if normalized(heard) != normalized(&full.visible_text()) {
        out.push(Cow::Owned(Message::new(
            Role::User,
            vec![ContentBlock::text(interruption_note(heard, approximate))],
        )));
    }
    out
}

/// Projekcja historii do wysłania: każda przerwana tura asystentki jest rozwijana wg strategii.
/// Wynik zawiera wszystkie oryginalne wiadomości w tej samej kolejności (pożyczone, bez kopii).
pub fn project_history(
    messages: &[Message],
    rendering: InterruptionRendering,
) -> Vec<Cow<'_, Message>> {
    let mut out = Vec::with_capacity(messages.len());
    for msg in messages {
        match (&msg.interruption, msg.role, rendering) {
            (Some(i), Role::Assistant, InterruptionRendering::AppendNote) => {
                out.extend(render_interrupted_turn(msg, &i.heard_prefix, i.approximate));
            }
            _ => out.push(Cow::Borrowed(msg)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{ProviderOrigin, ThinkingBlock};
    use proptest::prelude::*;

    fn thinking_turn(text: &str) -> Message {
        Message::new(
            Role::Assistant,
            vec![
                ContentBlock::Thinking(ThinkingBlock {
                    text: String::new(),
                    signature: Some("sig-abc".into()),
                    provider_origin: ProviderOrigin {
                        provider: "anthropic".into(),
                        model: "claude-opus-5-5".into(),
                    },
                }),
                ContentBlock::text(text),
            ],
        )
    }

    #[test]
    fn notes_cover_empty_approximate_and_full() {
        assert!(interruption_note("  ", false).contains("zanim usłyszał"));
        assert!(interruption_note("a", true).contains("w przybliżeniu"));
        let full = thinking_turn("Raz dwa trzy.");
        assert_eq!(
            render_interrupted_turn(&full, "Raz  dwa trzy.", false).len(),
            1
        );
        assert_eq!(render_interrupted_turn(&full, "", false).len(), 2);
    }

    #[test]
    fn projection_keeps_thinking_block_intact() {
        let full = thinking_turn("Najpierw A, potem B.").with_interruption("Najpierw A", true);
        let history = vec![
            Message::user_text("pytanie"),
            full.clone(),
            Message::user_text("stop"),
        ];
        let projected = project_history(&history, InterruptionRendering::AppendNote);
        assert_eq!(projected.len(), 4);
        assert_eq!(projected[1].as_ref(), &full);
        assert!(matches!(projected[1], Cow::Borrowed(_)));
        assert!(
            projected[2]
                .visible_text()
                .contains("(w przybliżeniu): „Najpierw A”")
        );
        let native = project_history(&history, InterruptionRendering::NativeTruncate);
        assert_eq!(native.len(), 3);
    }

    fn arb_message() -> impl Strategy<Value = Message> {
        (
            any::<bool>(),
            "[a-z ]{0,20}",
            proptest::option::of("[a-z ]{0,10}"),
        )
            .prop_map(|(assistant, text, heard)| {
                if assistant {
                    let m = thinking_turn(&text);
                    match heard {
                        Some(h) => m.with_interruption(h, false),
                        None => m,
                    }
                } else {
                    Message::user_text(text)
                }
            })
    }

    proptest! {
        /// Append-only: projekcja zawiera każdą oryginalną wiadomość bez zmian i w kolejności,
        /// a jedyne dodatki to notki użytkownika tuż po przerwanych turach.
        #[test]
        fn projection_is_append_only(history in proptest::collection::vec(arb_message(), 0..12)) {
            let projected = project_history(&history, InterruptionRendering::AppendNote);
            let mut j = 0;
            for original in &history {
                prop_assert_eq!(projected.get(j).map(|c| c.as_ref()), Some(original));
                j += 1;
                if let Some(i) = &original.interruption
                    && normalized(&i.heard_prefix) != normalized(&original.visible_text())
                {
                    let note = projected.get(j);
                    prop_assert_eq!(note.map(|n| n.role), Some(Role::User));
                    prop_assert!(note.is_some_and(|n| n.visible_text().starts_with("Użytkownik")));
                    j += 1;
                }
            }
            prop_assert_eq!(j, projected.len());
        }
    }
}
