//! Kontrola odpowiedzi końcowej przed weryfikacją (próby obowiązków na Bieliku 4.5B, 1.11).
//!
//! Dwie luki, których weryfikacja na tym samym małym modelu nie łapała:
//! - **odpowiedź urwana** — sam wstęp („Oto przegląd na dziś:”), pusta albo ucięta limitem tokenów;
//!   zadanie kończyło się „powodzeniem” z raportem bez treści;
//! - **źródła nieodczytane** — polecenie wskazuje strony (adresy `http(s)://`), a przebieg żadnej nie
//!   otworzył narzędziem sieciowym; model podawał wtedy „ceny” z pamięci.
//!
//! W obu przypadkach model dostaje jedną wiadomość z opisem luki i pracuje dalej. Liczba przypomnień
//! jest ograniczona ([`MAX_NUDGES`]) — po niej odpowiedź idzie do weryfikacji taka, jaka jest.

use providers_contract::{ContentBlock, Message};

use crate::catalog::{CALL, alias};

/// Najwięcej przypomnień o luce w jednym przebiegu.
pub(crate) const MAX_NUDGES: u8 = 2;

/// Narzędzia, które naprawdę czytają stronę albo wynik wyszukiwania.
const WEB_READERS: &[&str] = &[
    "net_fetch",
    "net_search",
    "browser_open",
    "browser_nav",
    "browser_read",
];

/// Luka w odpowiedzi końcowej.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Gap {
    /// Odpowiedź pusta, sam wstęp albo ucięta limitem tokenów.
    Unfinished,
    /// Polecenie wskazuje strony, a żadna nie została odczytana.
    UnreadSources {
        /// Czy przebieg ma narzędzie do czytania stron.
        can_read: bool,
    },
}

/// Najdłuższy fragment polecenia powtarzany w przypomnieniu o urwanej odpowiedzi (znaki).
const GOAL_IN_NOTE: usize = 600;

impl Gap {
    /// Wiadomość dla modelu (rola `User`, jak inne komunikaty wykonawcy). Przy urwanej odpowiedzi
    /// powtarza polecenie: samo „podaj cały wynik” mały model brał za nowe, niejasne zadanie
    /// i odpowiadał odmową niezwiązaną z celem (próba obowiązku „morning”, 1.11).
    pub(crate) fn note(&self, goal: &str) -> String {
        match self {
            Self::Unfinished => {
                let mut goal_part: String = goal.trim().chars().take(GOAL_IN_NOTE).collect();
                if goal.trim().chars().count() > GOAL_IN_NOTE {
                    goal_part.push('…');
                }
                format!(
                    "[Kontrola] Twoja odpowiedź jest niepełna — urywa się po wstępie albo jest pusta. \
                     Polecenie: „{goal_part}”. Podaj teraz cały gotowy wynik tego polecenia od początku, \
                     w całości, bez wstępu i bez zapowiedzi, na podstawie danych, które już masz."
                )
            }
            Self::UnreadSources { can_read: true } => {
                "[Kontrola] Polecenie wskazuje strony do sprawdzenia, a w tym zadaniu nie otworzyłaś \
                 żadnej. Odczytaj je narzędziem (net_fetch). Dane, których nie odczytasz w tym zadaniu, \
                 pomiń i napisz, że nie udało się ich sprawdzić — nie podawaj ich z pamięci."
                    .to_owned()
            }
            Self::UnreadSources { can_read: false } => {
                "[Kontrola] Polecenie wskazuje strony do sprawdzenia, a w tym zadaniu nie masz narzędzia \
                 do otwierania stron. Nie podawaj żadnych danych z tych stron ani z pamięci — napisz \
                 krótko, że sprawdzenie się nie udało, bo zadanie nie ma dostępu do sieci."
                    .to_owned()
            }
        }
    }
}

/// Luka w odpowiedzi `text` (kolejność: najpierw urwana, potem źródła).
pub(crate) fn gap(
    text: &str,
    truncated: bool,
    goal: &str,
    messages: &[Message],
    can_read: bool,
) -> Option<Gap> {
    if unfinished(text, truncated) {
        return Some(Gap::Unfinished);
    }
    if has_url(goal) && !read_web(messages) {
        return Some(Gap::UnreadSources { can_read });
    }
    None
}

/// Czy odpowiedź jest urwana: pusta, ucięta limitem albo krótki wstęp kończący się dwukropkiem.
pub(crate) fn unfinished(text: &str, truncated: bool) -> bool {
    let body = text.trim().trim_end_matches(['*', '_', '#', ' ']);
    if body.is_empty() || truncated {
        return true;
    }
    // Wstęp bez treści: jedna linia, kończy się „:” (lista miała dopiero nastąpić).
    body.ends_with(':') && body.lines().count() <= 2 && body.chars().count() <= 160
}

/// Czy w poleceniu jest adres strony.
pub(crate) fn has_url(goal: &str) -> bool {
    goal.contains("http://") || goal.contains("https://")
}

/// Czy przebieg odczytał stronę: wywołanie narzędzia sieciowego zakończone bez błędu.
pub(crate) fn read_web(messages: &[Message]) -> bool {
    let calls: Vec<&str> = messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|b| match b {
            ContentBlock::ToolUse(u) if is_web_reader(&u.name, &u.input) => Some(u.id.as_str()),
            _ => None,
        })
        .collect();
    messages
        .iter()
        .flat_map(|m| &m.content)
        .any(|b| matches!(b, ContentBlock::ToolResult(r) if !r.is_error && calls.contains(&r.tool_use_id.as_str())))
}

/// Narzędzie czytające sieć — także przez skrót `tool_call` („fetch”).
pub(crate) fn is_web_reader(name: &str, input: &serde_json::Value) -> bool {
    if WEB_READERS.contains(&name) {
        return true;
    }
    name == CALL
        && input
            .get("command")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|c| WEB_READERS.contains(&alias(&c.trim().to_lowercase())))
}

/// Czy rejestr przebiegu ma narzędzie do czytania stron.
pub(crate) fn can_read(names: &[String]) -> bool {
    names.iter().any(|n| WEB_READERS.contains(&n.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use providers_contract::{Role, ToolResult, ToolResultPart, ToolUse};
    use serde_json::json;

    fn call(id: &str, name: &str, input: serde_json::Value) -> Message {
        Message::new(
            Role::Assistant,
            vec![ContentBlock::ToolUse(ToolUse {
                id: id.into(),
                name: name.into(),
                input,
            })],
        )
    }

    fn result(id: &str, is_error: bool) -> Message {
        Message::new(
            Role::User,
            vec![ContentBlock::ToolResult(ToolResult {
                tool_use_id: id.into(),
                content: vec![ToolResultPart::Text { text: "…".into() }],
                is_error,
            })],
        )
    }

    #[test]
    fn intro_without_content_is_unfinished() {
        assert!(unfinished("Oto przegląd na dziś:", false));
        assert!(unfinished("**Przegląd na dziś:**", false));
        assert!(unfinished("   ", false));
        assert!(
            unfinished("1. Zadania: brak", true),
            "ucięta limitem tokenów"
        );
    }

    #[test]
    fn full_report_is_finished() {
        assert!(!unfinished(
            "1. Zadania otwarte: brak\n2. Koszty: 0,00 zł",
            false
        ));
        // Lista z nagłówkiem i treścią kończąca się dwukropkiem w środku nie jest wstępem.
        assert!(!unfinished("Przegląd:\n1. brak\n2. brak\n3. Uwagi:", false));
    }

    #[test]
    fn urls_in_goal_need_a_successful_read() {
        let goal = "Sprawdź ceny: https://openrouter.ai/api/v1/models";
        assert_eq!(
            gap("1. GPT: 1000", false, goal, &[], true),
            Some(Gap::UnreadSources { can_read: true })
        );
        let failed = [call("a", "net_fetch", json!({})), result("a", true)];
        assert!(
            gap("raport", false, goal, &failed, true).is_some(),
            "błąd odczytu się nie liczy"
        );
        let read = [call("b", "net_fetch", json!({})), result("b", false)];
        assert_eq!(gap("raport", false, goal, &read, true), None);
    }

    #[test]
    fn short_command_fetch_counts_as_read() {
        let read = [
            call("c", CALL, json!({"command": "fetch", "args": {}})),
            result("c", false),
        ];
        assert!(read_web(&read));
    }

    #[test]
    fn goal_without_urls_needs_no_read() {
        assert_eq!(
            gap(
                "1. brak",
                false,
                "Przegląd dnia z danych programu",
                &[],
                false
            ),
            None
        );
    }

    #[test]
    fn notes_tell_what_to_do() {
        let note = Gap::Unfinished.note("Przygotuj przegląd na dziś.");
        assert!(note.contains("cały gotowy wynik") && note.contains("Przygotuj przegląd na dziś."));
        let long = Gap::Unfinished.note(&"x".repeat(2000));
        assert!(long.chars().count() < 900, "polecenie przycięte");
        assert!(
            Gap::UnreadSources { can_read: false }
                .note("")
                .contains("nie ma dostępu do sieci")
        );
        assert!(can_read(&["fs_read".into(), "net_fetch".into()]));
        assert!(!can_read(&["fs_read".into()]));
    }
}
