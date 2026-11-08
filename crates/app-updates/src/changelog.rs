//! Krótkie „Co nowego” z dziennika zmian wbudowanego w program.
//!
//! Notatki wydania przychodzą w podpisanej paczce (`notes.md`). Budowa lokalna i wersje bez notatek
//! pokazywały samo „Zaktualizowano do wersji X”; wtedy bierzemy wpis tej wersji z `CHANGELOG.md`:
//! zdanie wstępu i po pierwszym zdaniu każdej pozycji, jako zwykły tekst.

/// Dziennik zmian z korzenia repozytorium, wkompilowany w program.
pub(crate) const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// Sekcje dla osób rozwijających program — pomijane w oknie „Co nowego”.
pub(crate) const SKIPPED: [&str; 2] = ["Testy", "Przyczyna błędu"];

/// Limit długości — okno ma pokazać skrót, nie cały wpis.
const MAX_CHARS: usize = 1600;

/// Dopisek, gdy wpis nie zmieścił się w limicie.
const MORE: &str = "\n… i więcej — pełna lista w dzienniku zmian (CHANGELOG.md).";

/// Tekst przed dwukropkiem krótszy niż tyle znaków to etykieta pozycji, nie zdanie.
const LABEL_CHARS: usize = 40;

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
    // Nagłówek sekcji czeka na swoją pierwszą pozycję — sekcja bez treści nie zostawia tytułu.
    let mut heading: Option<String> = None;
    for (kind, raw) in blocks {
        let text = plain(&raw);
        let line = match kind {
            Kind::Heading => {
                skipping = SKIPPED.contains(&text.as_str());
                heading = (!skipping).then(|| format!("\n{text}:\n"));
                continue;
            }
            Kind::Item => format!(
                "• {}",
                first_sentence(crate::history::split_severity(&text).1)
            ),
            Kind::Text => text,
        };
        if skipping {
            continue;
        }
        let lead = heading.as_deref().unwrap_or_default();
        if out.chars().count() + lead.chars().count() + line.chars().count() > MAX_CHARS {
            // Limit: kończymy w tym miejscu i mówimy o tym wprost, zamiast po cichu gubić pozycje.
            out.push_str(MORE);
            break;
        }
        out.push_str(lead);
        heading = None;
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
pub(crate) fn plain(text: &str) -> String {
    text.replace("**", "").replace('`', "")
}

/// Pierwsze zdanie pozycji: do kropki, po której zaczyna się nowe zdanie (wielka litera — „ok. 6 GB”
/// nie kończy zdania), albo do dwukropka zapowiadającego rozwinięcie. Krótki wstęp przed
/// dwukropkiem („Szkło: …”, „Drobne: …”) to etykieta, nie zdanie — wtedy czytamy dalej.
fn first_sentence(text: &str) -> String {
    let starts_sentence = |at: usize| {
        text.get(at..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(char::is_uppercase)
    };
    let period = text
        .match_indices(". ")
        .find(|(at, _)| starts_sentence(at + 2))
        .map(|(at, _)| at + 1);
    let colon = text.match_indices(": ").map(|(at, _)| at).find(|at| {
        text.get(..*at)
            .is_some_and(|head| head.chars().count() >= LABEL_CHARS)
    });
    let end = match (period, colon) {
        (Some(p), Some(c)) => p.min(c),
        (Some(at), None) | (None, Some(at)) => at,
        (None, None) => text.len(),
    };
    let mut sentence = text
        .get(..end)
        .unwrap_or(text)
        .trim_end_matches(':')
        .trim()
        .to_owned();
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

    /// Etykieta przed dwukropkiem nie staje się całą pozycją („• Szkło.”), a skrót „ok.” nie kończy zdania.
    #[test]
    fn labels_and_abbreviations_do_not_cut_the_sentence() {
        assert_eq!(
            first_sentence("Szkło: półprzezroczyste tafle na barwnej poświacie. Bez rozmycia tła."),
            "Szkło: półprzezroczyste tafle na barwnej poświacie."
        );
        assert_eq!(
            first_sentence("Pobranie pakietu ok. 6 GB trwa kilka minut. Potem restart."),
            "Pobranie pakietu ok. 6 GB trwa kilka minut."
        );
        assert_eq!(
            first_sentence("Bez kropki na końcu"),
            "Bez kropki na końcu."
        );
    }

    /// Za długi wpis kończy się dopiskiem; żaden nagłówek nie zostaje bez pozycji.
    #[test]
    fn long_entry_ends_with_a_note_and_has_no_empty_headings() {
        let mut log = String::from("## 3.0.0\n\nWstęp.\n\n### Zmiany\n\n");
        for n in 0..80 {
            log.push_str(&format!(
                "- Pozycja numer {n} z dłuższym opisem zmiany w programie.\n"
            ));
        }
        log.push_str("\n### Na końcu\n\n- Ostatnia pozycja.\n");
        let notes = summarize(&log, "3.0.0").unwrap();
        assert!(notes.ends_with(MORE.trim()), "{notes}");
        assert!(notes.chars().count() <= MAX_CHARS + MORE.chars().count());
        assert!(!notes.contains("Na końcu"), "nagłówek bez pozycji");
        let empty = summarize("## 4.0.0\n\n### Testy\n\n- x\n\n### Puste\n", "4.0.0");
        assert_eq!(empty, None);
    }

    /// Każdy wpis prawdziwego dziennika daje skrót bez urwanych pozycji.
    #[test]
    fn every_real_entry_is_summarized_cleanly() {
        let versions: Vec<&str> = CHANGELOG
            .lines()
            .filter_map(|l| l.strip_prefix("## "))
            .filter_map(|rest| rest.split_whitespace().next())
            .collect();
        assert!(versions.len() >= 5);
        for version in versions {
            let Some(notes) = notes_for(version) else {
                continue;
            };
            for line in notes.lines().filter(|l| l.starts_with('•')) {
                assert!(
                    line.chars().count() >= 20,
                    "wersja {version}: urwana pozycja {line:?}"
                );
            }
            for (line, next) in notes.lines().zip(notes.lines().skip(1)) {
                assert!(
                    !(line.ends_with(':') && !line.starts_with('•') && !next.starts_with('•')),
                    "wersja {version}: nagłówek {line:?} bez pozycji"
                );
            }
        }
    }

    #[test]
    fn running_version_has_an_entry() {
        let version = env!("CARGO_PKG_VERSION");
        let notes = notes_for(version).unwrap_or_default();
        assert!(
            notes.contains('•'),
            "CHANGELOG.md nie ma wpisu z listą zmian dla wersji {version} — dopisz go przy podbiciu wersji"
        );
    }
}
