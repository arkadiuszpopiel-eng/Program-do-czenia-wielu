//! Krótkie „Co nowego” z dziennika zmian wbudowanego w program.
//!
//! Notatki wydania przychodzą w podpisanej paczce (`notes.md`). Budowa lokalna i wersje bez notatek
//! pokazywały samo „Zaktualizowano do wersji X”; wtedy bierzemy wpis tej wersji z `CHANGELOG.md`:
//! zdanie wstępu i po pierwszym zdaniu każdej pozycji, jako zwykły tekst.

/// Dziennik zmian z korzenia repozytorium, wkompilowany w program.
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// Sekcje dla osób rozwijających program — pomijane w oknie „Co nowego”.
const SKIPPED: [&str; 2] = ["Testy", "Przyczyna błędu"];

/// Limit długości — okno ma pokazać skrót, nie cały wpis.
const MAX_CHARS: usize = 1600;

/// Skrót wpisu `version` z wbudowanego dziennika zmian; `None`, gdy takiego wpisu nie ma.
pub(crate) fn notes_for(version: &str) -> Option<String> {
    summarize(CHANGELOG, version)
}

fn summarize(changelog: &str, version: &str) -> Option<String> {
    let mut lines = changelog.lines();
    lines.by_ref().find(|line| is_entry(line, version))?;
    // Akapity i pozycje wpisu; każda zawinięta linia dołącza do poprzedniej.
    let mut blocks: Vec<(Kind, String)> = Vec::new();
    let mut open = false;
    for line in lines.take_while(|line| !line.starts_with("## ")) {
        let text = line.trim();
        if text.is_empty() {
            open = false;
        } else if let Some(title) = text.strip_prefix("### ") {
            blocks.push((Kind::Heading, title.to_owned()));
            open = false;
        } else if let Some(item) = text.strip_prefix("- ") {
            blocks.push((Kind::Item, item.to_owned()));
            open = true;
        } else if let (true, Some((_, last))) = (open, blocks.last_mut()) {
            last.push(' ');
            last.push_str(text);
        } else {
            blocks.push((Kind::Text, text.to_owned()));
            open = true;
        }
    }
    let mut out = String::new();
    let mut skipping = false;
    for (kind, raw) in blocks {
        let text = plain(&raw);
        let line = match kind {
            Kind::Heading => {
                skipping = SKIPPED.contains(&text.as_str());
                format!("\n{text}:")
            }
            Kind::Item => format!("• {}", first_sentence(&text)),
            Kind::Text => text,
        };
        if skipping || out.chars().count() + line.chars().count() > MAX_CHARS {
            continue;
        }
        out.push_str(&line);
        out.push('\n');
    }
    let out = out.trim().to_owned();
    (!out.is_empty()).then_some(out)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Heading,
    Item,
    Text,
}

/// Nagłówek wpisu: `## 0.0.4` albo `## 0.0.4 — 2026-10-09`.
fn is_entry(line: &str, version: &str) -> bool {
    line.strip_prefix("## ")
        .and_then(|rest| rest.split_whitespace().next())
        .is_some_and(|found| found == version)
}

/// Zdejmuje znaczniki Markdown, których zwykły tekst nie potrzebuje.
fn plain(text: &str) -> String {
    text.replace("**", "").replace('`', "")
}

/// Pierwsze zdanie pozycji (do kropki przed spacją albo do dwukropka zapowiadającego wyliczenie).
fn first_sentence(text: &str) -> String {
    let end = text
        .match_indices(". ")
        .map(|(at, _)| at + 1)
        .chain(text.match_indices(": ").map(|(at, _)| at))
        .min()
        .unwrap_or(text.len());
    let mut sentence = text[..end].trim_end_matches(':').trim().to_owned();
    if !sentence.ends_with('.') {
        sentence.push('.');
    }
    sentence
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Dziennik\n\nWstęp pliku.\n\n## 1.2.0 — 2026-10-09\n\nPoprawka list wyboru.\n\n\
### Interfejs\n\n- **Listy wyboru** są rysowane przez Alfę, nie przez system: własna lista\n  opcji w stylu `interfejsu`.\n\
- Paski przewijania bez strzałek. Drugie zdanie znika.\n\n### Testy\n\n- Tego nie pokazujemy.\n\n\
### Znane ograniczenia\n\n- Brak rozmycia tła.\n\n## 1.1.0\n\nStarszy wpis.\n";

    #[test]
    fn entry_becomes_short_plain_text() {
        let notes = summarize(SAMPLE, "1.2.0").unwrap();
        assert_eq!(
            notes,
            "Poprawka list wyboru.\n\nInterfejs:\n• Listy wyboru są rysowane przez Alfę, nie przez system.\n\
• Paski przewijania bez strzałek.\n\nZnane ograniczenia:\n• Brak rozmycia tła."
        );
    }

    #[test]
    fn other_versions_and_missing_entry() {
        assert_eq!(summarize(SAMPLE, "1.1.0").as_deref(), Some("Starszy wpis."));
        assert_eq!(summarize(SAMPLE, "9.9.9"), None);
        assert_eq!(
            summarize(SAMPLE, "1.2"),
            None,
            "numer musi się zgadzać w całości"
        );
    }

    #[test]
    fn running_version_has_an_entry() {
        let version = env!("CARGO_PKG_VERSION");
        let notes = notes_for(version).unwrap_or_default();
        assert!(
            notes.contains('•'),
            "CHANGELOG.md nie ma wpisu z listą zmian dla wersji {version} — dopisz go przy podbiciu wersji"
        );
        assert!(notes.chars().count() <= MAX_CHARS);
    }
}
