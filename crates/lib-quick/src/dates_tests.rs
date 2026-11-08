use super::answer;
use chrono::NaiveDate;

fn a(text: &str) -> Option<String> {
    let now = NaiveDate::from_ymd_opt(2026, 10, 15)
        .and_then(|d| d.and_hms_opt(14, 30, 0))
        .unwrap_or_default();
    answer(text, now)
}

#[test]
fn days_until_and_since() {
    let christmas = Some("Do 24 grudnia 2026 (czwartek) zostało **70 dni**.");
    for q in [
        "ile dni do 24 grudnia",
        "ile zostalo dni do 24.12",
        "za ile dni 2026-12-24",
        "ile jeszcze do 24 grudnia 2026",
    ] {
        assert_eq!(a(q).as_deref(), christmas, "{q}");
    }
    assert_eq!(
        a("ile dni do 1.1").as_deref(),
        Some("Do 1 stycznia 2027 (piątek) zostało **78 dni**."),
        "data bez roku — najbliższa w przód"
    );
    assert_eq!(
        a("ile dni do 1 stycznia 2020").as_deref(),
        Some("1 stycznia 2020 (środa) było **2479 dni** temu.")
    );
    assert_eq!(
        a("za ile dni 15.10").as_deref(),
        Some("To dziś — 15 października 2026 (czwartek).")
    );
    assert_eq!(
        a("ile dni minelo od 1 wrzesnia").as_deref(),
        Some("Od 1 września 2026 (wtorek) minęło **44 dni**.")
    );
    assert_eq!(
        a("ile minelo dni od 14.10.2026").as_deref(),
        Some("Od 14 października 2026 (środa) minęło **1 dzień**.")
    );
    assert_eq!(
        a("ile dni do 29 lutego").as_deref(),
        Some("Do 29 lutego 2028 (wtorek) zostało **502 dni**."),
        "29 lutego — najbliższy rok przestępny"
    );
    assert!(
        a("ile dni od 1 stycznia 2030")
            .unwrap_or_default()
            .contains("będzie za")
    );
}

#[test]
fn weekday_of_a_date() {
    for q in [
        "jaki dzien tygodnia byl 1 stycznia 2000",
        "jakim dniem tygodnia byl 1.1.2000",
        "jaki to dzien tygodnia 2000-01-01",
    ] {
        assert_eq!(
            a(q).as_deref(),
            Some("1 stycznia 2000 to **sobota**."),
            "{q}"
        );
    }
    assert_eq!(
        a("jaki dzien tygodnia bedzie 29 lutego").as_deref(),
        Some("29 lutego 2028 to **wtorek**.")
    );
    assert_eq!(
        a("jaki dzien tygodnia bedzie 24 grudnia").as_deref(),
        Some("24 grudnia 2026 to **czwartek**.")
    );
}

#[test]
fn shifted_dates() {
    for (q, want) in [
        (
            "jaka data bedzie za 30 dni",
            "Za 30 dni: **14 listopada 2026 (sobota)**.",
        ),
        (
            "jaki dzien bedzie jutro",
            "Jutro: **16 października 2026 (piątek)**.",
        ),
        (
            "jaki dzien byl wczoraj",
            "Wczoraj: **14 października 2026 (środa)**.",
        ),
        (
            "jaka data byla 2 tygodnie temu",
            "2 tygodnie temu: **1 października 2026 (czwartek)**.",
        ),
        (
            "jaka data bedzie za 3 miesiace",
            "Za 3 miesiące: **15 stycznia 2027 (piątek)**.",
        ),
        (
            "jaki dzien bedzie za tydzien",
            "Za tydzień: **22 października 2026 (czwartek)**.",
        ),
        (
            "jaki dzien jest pojutrze",
            "Pojutrze: **17 października 2026 (sobota)**.",
        ),
    ] {
        assert_eq!(a(q).as_deref(), Some(want), "{q}");
    }
}

#[test]
fn other_questions_go_to_the_model() {
    for q in [
        "ile dni do swiat",
        "jaki dzien tygodnia byl 31 lutego 2000",
        "jaka data bedzie za 999999 dni",
        "co bedzie jutro",
        "ile dni ma tydzien",
        "ile dni do 32.13",
    ] {
        assert_eq!(a(q), None, "{q}");
    }
}
