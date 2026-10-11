//! Historia zmian z dziennika wbudowanego w program (Ustawienia → Aktualizacje → Historia zmian):
//! wszystkie wersje od najnowszej, każda ze zdaniem wstępu i pozycjami z wagą — krytyczna,
//! poważna albo zwykła. Wagę niesie znacznik na początku pozycji w `CHANGELOG.md`
//! (`[krytyczne]`, `[poważne]`; bez znacznika — zwykła). Sekcje dla osób rozwijających program
//! („Testy”, „Przyczyna błędu”) są pomijane, tak jak w oknie „Co nowego”.

use app_api::dto::{ChangeItem, ChangeLogEntry, ChangeSeverity};

use crate::changelog::{CHANGELOG, SKIPPED, plain};

/// Znaczniki wagi w dzienniku zmian (małe litery; porównanie bez rozróżniania wielkości).
const TAGS: [(&str, ChangeSeverity); 2] = [
    ("[krytyczne]", ChangeSeverity::Critical),
    ("[poważne]", ChangeSeverity::Serious),
];

/// Zdejmuje znacznik wagi z początku pozycji; bez znacznika pozycja jest zwykła.
pub(crate) fn split_severity(item: &str) -> (ChangeSeverity, &str) {
    let text = item.trim_start();
    for (tag, severity) in TAGS {
        let head = text.get(..tag.len()).unwrap_or_default();
        if head.to_lowercase() == tag {
            return (severity, text[tag.len()..].trim_start());
        }
    }
    (ChangeSeverity::Normal, text)
}

/// Cała historia z wbudowanego dziennika zmian, od najnowszej wersji.
pub(crate) fn history() -> Vec<ChangeLogEntry> {
    parse(CHANGELOG)
}

fn parse(changelog: &str) -> Vec<ChangeLogEntry> {
    let mut entries: Vec<ChangeLogEntry> = Vec::new();
    let mut section = String::new();
    let mut skipping = false;
    // Co ostatnio otwarto: pozycję (kolejne linie ją przedłużają), akapit wstępu albo nic.
    let mut open = Open::Nothing;
    for line in changelog.lines() {
        let text = line.trim();
        if let Some(head) = line.strip_prefix("## ") {
            let mut words = head.split_whitespace();
            let version = words.next().unwrap_or_default().to_owned();
            let date = words.rfind(|w| w.len() == 10 && w.as_bytes()[4] == b'-');
            entries.push(ChangeLogEntry {
                version,
                date: date.map(str::to_owned),
                summary: String::new(),
                items: Vec::new(),
            });
            section.clear();
            skipping = false;
            open = Open::Nothing;
            continue;
        }
        let Some(entry) = entries.last_mut() else {
            // Wstęp pliku przed pierwszą wersją.
            continue;
        };
        if text.is_empty() {
            open = Open::Nothing;
        } else if let Some(title) = text.strip_prefix("### ") {
            section = plain(title);
            skipping = SKIPPED.contains(&section.as_str());
            open = Open::Nothing;
        } else if skipping {
            // Sekcja pomijana w całości (także zawinięte linie pozycji).
        } else if let Some(item) = text.strip_prefix("- ") {
            let (severity, body) = split_severity(item);
            entry.items.push(ChangeItem {
                section: section.clone(),
                text: plain(body),
                severity,
            });
            open = Open::Item;
        } else if open == Open::Item {
            if let Some(last) = entry.items.last_mut() {
                last.text.push(' ');
                last.text.push_str(&plain(text));
            }
        } else if section.is_empty() {
            if !entry.summary.is_empty() {
                entry.summary.push(' ');
            }
            entry.summary.push_str(&plain(text));
            open = Open::Summary;
        }
    }
    entries.retain(|e| !e.version.is_empty() && (!e.summary.is_empty() || !e.items.is_empty()));
    entries
}

#[derive(Clone, Copy, PartialEq)]
enum Open {
    Nothing,
    Summary,
    Item,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Dziennik\n\nWstęp pliku.\n\n## 1.2.0 — 2026-10-09\n\nPoprawka list\nwyboru.\n\n\
### Interfejs\n\n- [krytyczne] **Kopia** nie powstaje bez szyfrowania,\n  gdy brak hasła.\n\
- [Poważne] Lista zamyka się sama.\n- Paski przewijania bez strzałek.\n\n### Testy\n\n- Tego nie pokazujemy,\n  ani tej linii.\n\n\
### Głos\n\n- Głośność `mowy`.\n\n## 1.1.0\n\nStarszy wpis.\n\n## 1.0.0 — 2026-01-02\n";

    #[test]
    fn entries_items_and_severities() {
        let h = parse(SAMPLE);
        assert_eq!(h.len(), 2, "wersja bez treści jest pomijana");
        let new = &h[0];
        assert_eq!(new.version, "1.2.0");
        assert_eq!(new.date.as_deref(), Some("2026-10-09"));
        assert_eq!(new.summary, "Poprawka list wyboru.");
        let got: Vec<(&str, ChangeSeverity, &str)> = new
            .items
            .iter()
            .map(|i| (i.section.as_str(), i.severity, i.text.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                (
                    "Interfejs",
                    ChangeSeverity::Critical,
                    "Kopia nie powstaje bez szyfrowania, gdy brak hasła."
                ),
                (
                    "Interfejs",
                    ChangeSeverity::Serious,
                    "Lista zamyka się sama."
                ),
                (
                    "Interfejs",
                    ChangeSeverity::Normal,
                    "Paski przewijania bez strzałek."
                ),
                ("Głos", ChangeSeverity::Normal, "Głośność mowy."),
            ]
        );
        assert_eq!(h[1].version, "1.1.0");
        assert_eq!(h[1].date, None);
        assert_eq!(h[1].summary, "Starszy wpis.");
    }

    #[test]
    fn tag_is_only_recognised_at_the_start() {
        assert_eq!(
            split_severity("[krytyczne] Tekst"),
            (ChangeSeverity::Critical, "Tekst")
        );
        assert_eq!(
            split_severity("Tekst [krytyczne] w środku"),
            (ChangeSeverity::Normal, "Tekst [krytyczne] w środku")
        );
        assert_eq!(
            split_severity("[inne] Tekst"),
            (ChangeSeverity::Normal, "[inne] Tekst")
        );
    }

    /// Prawdziwy dziennik: każda wersja ma numer, bieżąca wersja programu jest pierwsza, są
    /// pozycje każdej wagi, a żaden znacznik nie przecieka do tekstu.
    #[test]
    fn built_in_changelog_is_well_formed() {
        let h = history();
        assert!(h.len() >= 20, "{}", h.len());
        assert_eq!(h[0].version, env!("CARGO_PKG_VERSION"));
        for severity in [
            ChangeSeverity::Critical,
            ChangeSeverity::Serious,
            ChangeSeverity::Normal,
        ] {
            assert!(
                h.iter()
                    .flat_map(|e| &e.items)
                    .any(|i| i.severity == severity)
            );
        }
        for entry in &h {
            assert!(
                entry
                    .version
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit()),
                "{}",
                entry.version
            );
            for item in &entry.items {
                assert!(!item.text.is_empty());
                assert!(!item.text.starts_with('['), "{}", item.text);
                assert!(!item.section.is_empty(), "{}", item.text);
                assert!(!SKIPPED.contains(&item.section.as_str()));
            }
        }
    }
}
