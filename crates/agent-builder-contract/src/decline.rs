//! Odmiana imion żeńskich na „-a” przez przypadki (heurystyka PL; właściciel może poprawić
//! formy w formularzu). Imiona o innej końcówce — formy jednakowe.

use personas_contract::NameForms;

/// Rdzenie miękkie zdrobnień („Kasia”, „Ania”): D/C/Ms = rdzeń, W = rdzeń + „u”.
const SOFT_DIMINUTIVE: [&str; 5] = ["dzi", "si", "ni", "zi", "ci"];

/// Zmiękczenie spółgłoski w celowniku/miejscowniku (z końcówką).
const SOFTEN: [(&str, &str); 19] = [
    ("st", "ście"),
    ("zd", "ździe"),
    ("sł", "śle"),
    ("sn", "śnie"),
    ("ch", "sze"),
    ("t", "cie"),
    ("d", "dzie"),
    ("r", "rze"),
    ("k", "ce"),
    ("g", "dze"),
    ("ł", "le"),
    ("s", "sie"),
    ("z", "zie"),
    ("n", "nie"),
    ("m", "mie"),
    ("b", "bie"),
    ("p", "pie"),
    ("w", "wie"),
    ("f", "fie"),
];

/// Spółgłoski funkcjonalnie miękkie (D = C = Ms z „-y”).
const HARDENED: [&str; 7] = ["cz", "sz", "rz", "ż", "dż", "dz", "c"];

fn strip_suffix<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    s.strip_suffix(suffix)
}

/// Formy imienia (mianownik, dopełniacz, celownik, biernik, narzędnik, miejscownik, wołacz).
pub fn decline_feminine(name: &str) -> NameForms {
    let name = name.trim();
    let Some(stem) = strip_suffix(name, "a").filter(|s| s.chars().count() >= 1) else {
        return NameForms::uniform(name);
    };
    let acc = format!("{stem}ę");
    let ins = format!("{stem}ą");
    let lower = stem.to_lowercase();
    let (gen_, dat, voc) = if SOFT_DIMINUTIVE.iter().any(|s| lower.ends_with(s)) {
        (stem.to_owned(), stem.to_owned(), format!("{stem}u"))
    } else if lower.ends_with('i') {
        let g = format!("{stem}i");
        (g.clone(), g, format!("{stem}o"))
    } else if lower.ends_with('j') {
        let base: String = {
            let mut c = stem.chars();
            c.next_back();
            c.as_str().to_owned()
        };
        let g = format!("{base}i");
        (g.clone(), g, format!("{stem}o"))
    } else if lower.ends_with('l') {
        let g = format!("{stem}i");
        (g.clone(), g, format!("{stem}u"))
    } else if HARDENED.iter().any(|h| lower.ends_with(h)) {
        let g = format!("{stem}y");
        (g.clone(), g, format!("{stem}o"))
    } else {
        let genitive = if lower.ends_with('k') || lower.ends_with('g') {
            format!("{stem}i")
        } else {
            format!("{stem}y")
        };
        let dative = SOFTEN
            .iter()
            .find_map(|(from, to)| {
                if !lower.ends_with(from) {
                    return None;
                }
                stem.get(..stem.len().saturating_sub(from.len()))
                    .map(|base| format!("{base}{to}"))
            })
            .unwrap_or_else(|| genitive.clone());
        (genitive, dative, format!("{stem}o"))
    };
    NameForms {
        nominative: name.to_owned(),
        genitive: gen_,
        dative: dat.clone(),
        accusative: acc,
        instrumental: ins,
        locative: dat,
        vocative: voc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use personas_contract::builtin_personas;

    fn forms(n: &str) -> [String; 7] {
        let f = decline_feminine(n);
        [
            f.nominative,
            f.genitive,
            f.dative,
            f.accusative,
            f.instrumental,
            f.locative,
            f.vocative,
        ]
    }

    #[test]
    fn builtin_names_match_catalog() {
        for p in builtin_personas() {
            assert_eq!(decline_feminine(&p.name), p.forms, "{}", p.name);
        }
    }

    #[test]
    fn common_names() {
        let table: [(&str, [&str; 7]); 9] = [
            ("Ola", ["Ola", "Oli", "Oli", "Olę", "Olą", "Oli", "Olu"]),
            (
                "Kasia",
                ["Kasia", "Kasi", "Kasi", "Kasię", "Kasią", "Kasi", "Kasiu"],
            ),
            (
                "Kinga",
                [
                    "Kinga", "Kingi", "Kindze", "Kingę", "Kingą", "Kindze", "Kingo",
                ],
            ),
            (
                "Marta",
                [
                    "Marta", "Marty", "Marcie", "Martę", "Martą", "Marcie", "Marto",
                ],
            ),
            (
                "Klara",
                [
                    "Klara", "Klary", "Klarze", "Klarę", "Klarą", "Klarze", "Klaro",
                ],
            ),
            (
                "Julia",
                [
                    "Julia", "Julii", "Julii", "Julię", "Julią", "Julii", "Julio",
                ],
            ),
            (
                "Maja",
                ["Maja", "Mai", "Mai", "Maję", "Mają", "Mai", "Majo"],
            ),
            (
                "Agnieszka",
                [
                    "Agnieszka",
                    "Agnieszki",
                    "Agnieszce",
                    "Agnieszkę",
                    "Agnieszką",
                    "Agnieszce",
                    "Agnieszko",
                ],
            ),
            (
                "Zeta",
                ["Zeta", "Zety", "Zecie", "Zetę", "Zetą", "Zecie", "Zeto"],
            ),
        ];
        for (name, want) in table {
            assert_eq!(forms(name), want.map(str::to_owned), "{name}");
        }
        assert_eq!(decline_feminine("Nel"), NameForms::uniform("Nel"));
    }
}
