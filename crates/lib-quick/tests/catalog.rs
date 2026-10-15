//! Katalog poleceń bez modelu: każdy przykład trafia we własne polecenie, a zwykłe pytania
//! (także podobne z wyglądu) idą do modelu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::NaiveDate;
use lib_quick::{CATALOG, Command, answer, matches};

fn now() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 15)
        .and_then(|d| d.and_hms_opt(14, 30, 0))
        .unwrap()
}

#[test]
fn every_example_triggers_its_command() {
    for entry in CATALOG {
        for example in entry.examples {
            if entry.command == Command::Quiz {
                let (_, first) = lib_quick::quiz::start(example, &mut |n| n / 2)
                    .unwrap_or_else(|| panic!("brak quizu: {example}"));
                assert!(first.contains("= ?"), "{example}");
                assert!(!matches(example), "quiz nie jest poleceniem jednorazowym");
                continue;
            }
            let reply = answer(example, now(), &mut |n| n / 2)
                .unwrap_or_else(|| panic!("brak odpowiedzi: {example}"));
            assert_eq!(reply.command, entry.command, "{example}");
            assert!(!reply.text.is_empty());
            assert!(matches(example), "{example}");
        }
    }
}

#[test]
fn ordinary_questions_go_to_the_model() {
    for text in [
        "Ile kosztuje bilet do Krakowa?",
        "Która godzina jest teraz w Tokio?",
        "Jak się pisze list motywacyjny?",
        "Napisz funkcję, która liczy 2+2 w Pythonie",
        "Podaj liczbę mieszkańców Polski",
        "Rzuć okiem na ten plik",
        "Jaki dzień tygodnia był 1 stycznia 2000?",
        "Opowiedz o roku 1984",
        "ile to jest\n7 razy 8",
        "",
    ] {
        assert_eq!(answer(text, now(), &mut |_| 0), None, "{text}");
    }
}

#[test]
fn replies_are_exact() {
    let a = |text: &str| answer(text, now(), &mut |_| 0).map(|r| r.text);
    assert_eq!(
        a("Ile to jest 7 razy 8?").as_deref(),
        Some("7 × 8 = **56**")
    );
    assert_eq!(
        a("dwadzieścia trzy plus sto").as_deref(),
        Some("23 + 100 = **123**")
    );
    assert_eq!(a("Która godzina?").as_deref(), Some("Jest godzina 14:30."));
    assert_eq!(
        a("Hej Alfa, jaki dziś dzień?").as_deref(),
        Some("Dziś jest czwartek, 15 października 2026.")
    );
    assert_eq!(a("rzuć kostką").as_deref(), Some("Wypadło: **1**."));
    assert_eq!(
        answer("2x3", now(), &mut |_| 0).map(|r| r.command),
        Some(Command::Math)
    );
}
