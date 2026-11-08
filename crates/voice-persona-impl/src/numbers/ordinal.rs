//! Liczebniki porządkowe (daty, lata, godziny) w potrzebnych formach przymiotnikowych.

use super::{HUNDREDS, cardinal_nom, idx};

/// Forma przymiotnikowa liczebnika porządkowego.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrdForm {
    /// Mianownik m. („pierwszy”).
    NomM,
    /// Dopełniacz m. („pierwszego”) — daty, rok.
    GenM,
    /// Miejscownik m. („w dwa tysiące dwudziestym szóstym”).
    LocM,
    /// Mianownik ż. („czternasta”) — godziny.
    NomF,
    /// Dopełniacz/miejscownik ż. („o czternastej”).
    GenLocF,
    /// Narzędnik/biernik ż. („przed czternastą”, „na czternastą”).
    InsAccF,
}

#[derive(Clone, Copy)]
enum Stem {
    Hard(&'static str),
    Velar(&'static str),
    Soft(&'static str),
}

const ORD_UNITS: [Stem; 20] = [
    Stem::Hard("zerow"),
    Stem::Hard("pierwsz"),
    Stem::Velar("drug"),
    Stem::Soft("trzec"),
    Stem::Hard("czwart"),
    Stem::Hard("piąt"),
    Stem::Hard("szóst"),
    Stem::Hard("siódm"),
    Stem::Hard("ósm"),
    Stem::Hard("dziewiąt"),
    Stem::Hard("dziesiąt"),
    Stem::Hard("jedenast"),
    Stem::Hard("dwunast"),
    Stem::Hard("trzynast"),
    Stem::Hard("czternast"),
    Stem::Hard("piętnast"),
    Stem::Hard("szesnast"),
    Stem::Hard("siedemnast"),
    Stem::Hard("osiemnast"),
    Stem::Hard("dziewiętnast"),
];
const ORD_TENS: [&str; 10] = [
    "",
    "",
    "dwudziest",
    "trzydziest",
    "czterdziest",
    "pięćdziesiąt",
    "sześćdziesiąt",
    "siedemdziesiąt",
    "osiemdziesiąt",
    "dziewięćdziesiąt",
];
const ORD_HUNDREDS: [&str; 10] = [
    "",
    "setn",
    "dwusetn",
    "trzechsetn",
    "czterechsetn",
    "pięćsetn",
    "sześćsetn",
    "siedemsetn",
    "osiemsetn",
    "dziewięćsetn",
];
const ORD_THOUSANDS: [&str; 10] = [
    "",
    "tysięczn",
    "dwutysięczn",
    "trzytysięczn",
    "czterotysięczn",
    "pięciotysięczn",
    "sześciotysięczn",
    "siedmiotysięczn",
    "ośmiotysięczn",
    "dziewięciotysięczn",
];

fn inflect(stem: Stem, form: OrdForm) -> String {
    let (base, endings): (&str, [&str; 6]) = match stem {
        Stem::Hard(b) => (b, ["y", "ego", "ym", "a", "ej", "ą"]),
        Stem::Velar(b) => (b, ["i", "iego", "im", "a", "iej", "ą"]),
        Stem::Soft(b) => (b, ["i", "iego", "im", "ia", "iej", "ią"]),
    };
    let i = match form {
        OrdForm::NomM => 0,
        OrdForm::GenM => 1,
        OrdForm::LocM => 2,
        OrdForm::NomF => 3,
        OrdForm::GenLocF => 4,
        OrdForm::InsAccF => 5,
    };
    format!("{base}{}", endings[i])
}

fn ord_below_100(n: u64, form: OrdForm, out: &mut Vec<String>) {
    if n < 20 {
        out.push(inflect(ORD_UNITS[usize::try_from(n).unwrap_or(0)], form));
        return;
    }
    out.push(inflect(Stem::Hard(ORD_TENS[idx(n / 10)]), form));
    if !n.is_multiple_of(10) {
        out.push(inflect(ORD_UNITS[idx(n)], form));
    }
}

/// Liczebnik porządkowy 0–9999 (daty, lata, godziny). Większe → `None`.
pub fn ordinal(n: u64, form: OrdForm) -> Option<String> {
    if n > 9999 {
        return None;
    }
    let (th, r) = (n / 1000, n % 1000);
    if th > 0 && r == 0 {
        return Some(inflect(Stem::Hard(ORD_THOUSANDS[idx(th)]), form));
    }
    let mut out = Vec::new();
    if th > 0 {
        out.push(cardinal_nom(th * 1000));
    }
    let (h, t) = (r / 100, r % 100);
    if t == 0 && h > 0 {
        out.push(inflect(Stem::Hard(ORD_HUNDREDS[idx(h)]), form));
    } else {
        if h > 0 {
            out.push(HUNDREDS[idx(h)].to_owned());
        }
        ord_below_100(t, form, &mut out);
    }
    Some(out.join(" "))
}
