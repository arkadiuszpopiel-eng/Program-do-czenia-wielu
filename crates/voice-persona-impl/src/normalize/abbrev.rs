//! Skróty (np., itd., m.in., tzn., dr, ul., godz. …) z odmianą zależną od przyimka.

use super::tokens::{Tok, dot_is_abbreviation, skip_space};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Skrót w środku zdania (kropka zawsze należy do skrótu).
    Mid,
    /// Skrót, który może kończyć zdanie (itd., itp.) — kropka zostaje, gdy dalej jest nowe zdanie.
    Final,
}

#[derive(Clone, Copy)]
enum Forms {
    Fixed(&'static str),
    /// Mianownik, dopełniacz, miejscownik, narzędnik, biernik.
    Cased([&'static str; 5]),
}

#[derive(Clone, Copy)]
struct Abbr {
    parts: &'static [&'static str],
    forms: Forms,
    kind: Kind,
    dot_optional: bool,
    person: bool,
    needs_number: bool,
}

const fn fixed(parts: &'static [&'static str], text: &'static str, kind: Kind) -> Abbr {
    Abbr {
        parts,
        forms: Forms::Fixed(text),
        kind,
        dot_optional: false,
        person: false,
        needs_number: false,
    }
}

const fn cased(parts: &'static [&'static str], forms: [&'static str; 5], person: bool) -> Abbr {
    Abbr {
        parts,
        forms: Forms::Cased(forms),
        kind: Kind::Mid,
        dot_optional: false,
        person,
        needs_number: false,
    }
}

const fn optional_dot(mut a: Abbr) -> Abbr {
    a.dot_optional = true;
    a
}

use Kind::{Final, Mid};

const TABLE: &[Abbr] = &[
    fixed(&["np"], "na przykład", Mid),
    fixed(&["itd"], "i tak dalej", Final),
    fixed(&["itp"], "i tym podobne", Final),
    fixed(&["m", "in"], "między innymi", Mid),
    fixed(&["tzn"], "to znaczy", Mid),
    fixed(&["tj"], "to jest", Mid),
    fixed(&["tzw"], "tak zwany", Mid),
    optional_dot(fixed(&["wg"], "według", Mid)),
    optional_dot(cased(
        &["dr"],
        ["doktor", "doktora", "doktorze", "doktorem", "doktora"],
        true,
    )),
    cased(
        &["prof"],
        [
            "profesor",
            "profesora",
            "profesorze",
            "profesorem",
            "profesora",
        ],
        true,
    ),
    optional_dot(fixed(&["mgr"], "magister", Mid)),
    fixed(&["inż"], "inżynier", Mid),
    cased(
        &["ul"],
        ["ulica", "ulicy", "ulicy", "ulicą", "ulicę"],
        false,
    ),
    cased(&["al"], ["aleja", "alei", "alei", "aleją", "aleję"], false),
    cased(
        &["godz"],
        ["godzina", "godziny", "godzinie", "godziną", "godzinę"],
        false,
    ),
    optional_dot(cased(
        &["nr"],
        ["numer", "numeru", "numerze", "numerem", "numer"],
        false,
    )),
    cased(
        &["tel"],
        ["telefon", "telefonu", "telefonie", "telefonem", "telefon"],
        false,
    ),
    optional_dot(cased(
        &["pkt"],
        ["punkt", "punktu", "punkcie", "punktem", "punkt"],
        false,
    )),
    fixed(&["zob"], "zobacz", Mid),
    fixed(&["ds"], "do spraw", Mid),
    fixed(&["ww"], "wyżej wymieniony", Mid),
    fixed(&["jw"], "jak wyżej", Final),
    fixed(&["cdn"], "ciąg dalszy nastąpi", Final),
    fixed(&["ew"], "ewentualnie", Mid),
    fixed(&["św"], "święty", Mid),
    fixed(&["br"], "bieżącego roku", Final),
    fixed(&["ang"], "angielski", Mid),
    fixed(&["etc"], "et cetera", Final),
    fixed(&["p", "n", "e"], "przed naszą erą", Final),
    fixed(&["n", "e"], "naszej ery", Final),
    fixed(&["tys"], "tysięcy", Final),
    optional_dot(fixed(&["mln"], "milionów", Final)),
    optional_dot(fixed(&["mld"], "miliardów", Final)),
    Abbr {
        parts: &["ok"],
        forms: Forms::Fixed("około"),
        kind: Mid,
        dot_optional: false,
        person: false,
        needs_number: true,
    },
];

fn shape_ok(word: &str, part: &str) -> Option<bool> {
    let lower = word.to_lowercase();
    if lower != part {
        return None;
    }
    let mut chars = word.chars();
    let first_upper = chars.next().is_some_and(char::is_uppercase);
    let rest_lower = chars.all(|c| !c.is_uppercase());
    rest_lower.then_some(first_upper)
}

fn case_index(prev: Option<&str>, abbr: &Abbr) -> usize {
    let godz = abbr.parts == ["godz"];
    match prev {
        Some("na") if godz => 4,
        Some("na" | "przy" | "o" | "po" | "w" | "we") => 2,
        Some("z" | "ze") if abbr.person => 3,
        Some(
            "do" | "od" | "koło" | "obok" | "u" | "około" | "bez" | "dla" | "z" | "ze" | "spod"
            | "sprzed" | "wzdłuż",
        ) => 1,
        Some("przed" | "za" | "nad" | "pod" | "między" | "pomiędzy") => 3,
        _ => 0,
    }
}

/// Czy po pozycji `k` zaczyna się nowe zdanie (koniec tekstu, nowa linia, wielka litera).
fn sentence_follows(toks: &[Tok], k: usize) -> bool {
    match toks.get(k) {
        None => true,
        Some(Tok::Space(s)) if s.contains('\n') => true,
        Some(Tok::Space(_)) => match toks.get(k + 1) {
            None => true,
            Some(Tok::Word(w)) => w.chars().next().is_some_and(char::is_uppercase),
            _ => false,
        },
        _ => false,
    }
}

fn try_abbr(toks: &[Tok], i: usize, abbr: &Abbr, prev: Option<&str>) -> Option<(usize, String)> {
    let capital = shape_ok(toks.get(i)?.word()?, abbr.parts.first()?)?;
    let mut j = i + 1;
    for part in abbr.parts.iter().skip(1) {
        if !toks.get(j).is_some_and(|t| t.is_sym('.')) {
            return None;
        }
        shape_ok(toks.get(j + 1)?.word()?, part)?;
        j += 2;
    }
    let has_dot = toks.get(j).is_some_and(|t| t.is_sym('.'));
    if !has_dot && !abbr.dot_optional {
        return None;
    }
    if abbr.needs_number {
        let k = skip_space(toks, j + 1);
        if k == j + 1 || toks.get(k).and_then(Tok::num).is_none() {
            return None;
        }
    }
    let mut text = match abbr.forms {
        Forms::Fixed(t) => t.to_owned(),
        Forms::Cased(forms) => forms
            .get(case_index(prev, abbr))
            .copied()
            .unwrap_or(forms[0])
            .to_owned(),
    };
    if capital {
        let mut chars = text.chars();
        text = chars
            .next()
            .map(|c| c.to_uppercase().chain(chars).collect())
            .unwrap_or_default();
    }
    if !has_dot {
        return Some((j, text));
    }
    let keeps_period = match abbr.kind {
        Kind::Final => sentence_follows(toks, j + 1),
        Kind::Mid => {
            abbr.dot_optional && !dot_is_abbreviation(toks, j) && sentence_follows(toks, j + 1)
        }
    };
    if keeps_period {
        text.push('.');
    }
    Some((j + 1, text))
}

/// Reguła skrótów dla tokenu słowa.
pub(crate) fn rule(toks: &[Tok], i: usize, prev: Option<&str>) -> Option<(usize, String)> {
    let before = i.checked_sub(1).and_then(|p| toks.get(p));
    if matches!(
        before,
        Some(Tok::Sym('.' | '@' | '/' | '-' | '_' | '\'' | '’'))
    ) {
        return None;
    }
    TABLE.iter().find_map(|abbr| try_abbr(toks, i, abbr, prev))
}
