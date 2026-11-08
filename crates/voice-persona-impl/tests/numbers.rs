//! Testy liczebników polskich (główne, dopełniacz, rodzaj, porządkowe).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use voice_persona_impl::numbers::*;

#[test]
fn cardinals_nominative() {
    let cases = [
        (0, "zero"),
        (1, "jeden"),
        (12, "dwanaście"),
        (21, "dwadzieścia jeden"),
        (100, "sto"),
        (215, "dwieście piętnaście"),
        (1000, "tysiąc"),
        (1001, "tysiąc jeden"),
        (2000, "dwa tysiące"),
        (5000, "pięć tysięcy"),
        (12_000, "dwanaście tysięcy"),
        (22_500, "dwadzieścia dwa tysiące pięćset"),
        (1_000_000, "milion"),
        (3_000_000, "trzy miliony"),
        (
            999_999_999_999,
            "dziewięćset dziewięćdziesiąt dziewięć miliardów dziewięćset dziewięćdziesiąt dziewięć milionów dziewięćset dziewięćdziesiąt dziewięć tysięcy dziewięćset dziewięćdziesiąt dziewięć",
        ),
    ];
    for (n, want) in cases {
        assert_eq!(cardinal_nom(n), want, "{n}");
    }
    assert_eq!(
        cardinal(1_000_000_000_000, NumCase::Nom, Gender::Masc),
        "jeden zero zero zero zero zero zero zero zero zero zero zero zero"
    );
}

#[test]
fn cardinals_gender_and_genitive() {
    assert_eq!(cardinal(1, NumCase::Nom, Gender::Fem), "jedna");
    assert_eq!(cardinal(1, NumCase::Nom, Gender::Neut), "jedno");
    assert_eq!(cardinal(2, NumCase::Nom, Gender::Fem), "dwie");
    assert_eq!(cardinal(22, NumCase::Nom, Gender::Fem), "dwadzieścia dwie");
    assert_eq!(
        cardinal(2002, NumCase::Nom, Gender::Fem),
        "dwa tysiące dwie"
    );
    assert_eq!(cardinal(21, NumCase::Nom, Gender::Fem), "dwadzieścia jeden");
    assert_eq!(cardinal(5, NumCase::Gen, Gender::Masc), "pięciu");
    assert_eq!(cardinal(1, NumCase::Gen, Gender::Fem), "jednej");
    assert_eq!(cardinal(21, NumCase::Gen, Gender::Masc), "dwudziestu jeden");
    assert_eq!(cardinal(1000, NumCase::Gen, Gender::Masc), "tysiąca");
    assert_eq!(
        cardinal(2500, NumCase::Gen, Gender::Masc),
        "dwóch tysięcy pięciuset"
    );
    assert_eq!(cardinal(0, NumCase::Gen, Gender::Masc), "zera");
}

#[test]
fn plural_rule() {
    assert_eq!(plural(1), Plural::One);
    assert_eq!(plural(3), Plural::Few);
    assert_eq!(plural(13), Plural::Many);
    assert_eq!(plural(24), Plural::Few);
    assert_eq!(plural(25), Plural::Many);
    assert_eq!(plural(0), Plural::Many);
}

#[test]
fn ordinals() {
    let o = |n, f| ordinal(n, f).unwrap();
    assert_eq!(o(1, OrdForm::GenM), "pierwszego");
    assert_eq!(o(2, OrdForm::GenM), "drugiego");
    assert_eq!(o(3, OrdForm::NomF), "trzecia");
    assert_eq!(o(31, OrdForm::GenM), "trzydziestego pierwszego");
    assert_eq!(o(14, OrdForm::NomF), "czternasta");
    assert_eq!(o(22, OrdForm::GenLocF), "dwudziestej drugiej");
    assert_eq!(o(23, OrdForm::InsAccF), "dwudziestą trzecią");
    assert_eq!(o(2026, OrdForm::GenM), "dwa tysiące dwudziestego szóstego");
    assert_eq!(o(2026, OrdForm::LocM), "dwa tysiące dwudziestym szóstym");
    assert_eq!(
        o(1999, OrdForm::GenM),
        "tysiąc dziewięćset dziewięćdziesiątego dziewiątego"
    );
    assert_eq!(o(2000, OrdForm::GenM), "dwutysięcznego");
    assert_eq!(o(1900, OrdForm::NomM), "tysiąc dziewięćsetny");
    assert_eq!(o(2100, OrdForm::GenM), "dwa tysiące setnego");
    assert_eq!(ordinal(10_000, OrdForm::NomM), None);
}

#[test]
fn digit_strings() {
    assert_eq!(digits("007"), "zero zero siedem");
    assert_eq!(
        read_digit_string("007", NumCase::Nom, Gender::Masc),
        "zero zero siedem"
    );
    assert_eq!(
        read_digit_string("42", NumCase::Nom, Gender::Masc),
        "czterdzieści dwa"
    );
    assert_eq!(
        read_digit_string("99999999999999999999999", NumCase::Nom, Gender::Masc)
            .split(' ')
            .count(),
        23
    );
}
