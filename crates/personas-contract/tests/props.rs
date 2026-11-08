//! Testy własności (ACC-F2-personas-02): imię zawsze wygrywa; bez imienia → Dyrygentka;
//! polecenia obsady zachowują poprawność obsady; serde round-trip.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use personas_contract::{
    Cast, CastCommand, Catalog, PersonaId, PersonasExport, RoleId, TemplateId, apply_command,
    builtin_personas, find_mentions, parse_addressee, parse_cast_command, resolve_addressee,
    tokenize,
};
use proptest::prelude::*;

const FILLER: &[&str] = &[
    "co",
    "masz",
    "na",
    "dziś",
    "sprawdź",
    "plik",
    "zrób",
    "to",
    "szybko",
    "i",
    "podsumuj",
    "wyniki",
    "jutro",
    "raport",
    "kod",
    "Kraków",
    "123",
    "okno",
    "zadanie",
    "dlaczego",
    "tak",
    "nie",
    "teraz",
    "ty",
    "prowadzisz",
    "weryfikację",
    "?",
    ".",
    ",",
    "!",
];

/// Formy innych person, których nie da się użyć jako zwrotu (celownik, dopełniacz, biernik…).
const OTHER_FORMS: &[&str] = &[
    "Alfie", "Alfy", "Alfę", "Becie", "Bety", "Betę", "Gamie", "Gamy", "Gamę", "Delcie", "Delty",
    "Deltę",
];

fn filler(with_others: bool) -> impl Strategy<Value = String> {
    let words: Vec<&'static str> = if with_others {
        FILLER.iter().chain(OTHER_FORMS).copied().collect()
    } else {
        FILLER.to_vec()
    };
    prop::collection::vec(prop::sample::select(words), 0..12).prop_map(|w| w.join(" "))
}

fn persona_index() -> impl Strategy<Value = usize> {
    0usize..4
}

/// Styl zwrotu: 0 „Imię, …”, 1 „Wołacz …”, 2 „hej Imię …”, 3 „@id …”, 4 „…, Wołacz, …”, 5 „…, Imię?”.
fn address(style: u8, idx: usize, rest: &str) -> String {
    let p = &builtin_personas()[idx];
    match style {
        0 => format!("{}, {rest}", p.name),
        1 => format!("{} {rest}", p.forms.vocative),
        2 => format!("hej {} {rest}", p.name),
        3 => format!("@{} {rest}", p.id),
        4 => format!("zrób to, {}, {rest}", p.forms.vocative),
        _ => format!("{rest}, {}?", p.name),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Imię zawsze wygrywa — także gdy w reszcie są inne persony w formach niezwrotnych.
    #[test]
    fn name_always_wins(idx in persona_index(), style in 0u8..6, rest in filler(true)) {
        let personas = builtin_personas();
        // Styl 4 i 5 (wtrącenie) — reszta bez innych imion, żeby zwrot był jedyny.
        let rest = if style >= 4 { rest.split(' ').filter(|w| !OTHER_FORMS.contains(w)).collect::<Vec<_>>().join(" ") } else { rest };
        let text = address(style, idx, &rest);
        prop_assert_eq!(parse_addressee(&text, &personas), Some(personas[idx].id.clone()), "{}", text);
    }

    /// Bez imienia odpowiada Dyrygentka obsady (dowolnej poprawnej).
    #[test]
    fn no_name_means_conductor(rest in filler(false), conductor in persona_index(), tpl in 0usize..4) {
        let catalog = Catalog::builtin();
        let personas = builtin_personas();
        prop_assume!(find_mentions(&tokenize(&rest), &personas).is_empty());
        let template = catalog.templates()[tpl].id.clone();
        let mut cast = catalog.cast_from_template(&template, None, false).unwrap();
        cast.assign(&personas[conductor].id, &RoleId::conductor(), true);
        prop_assert!(catalog.validate_cast(&cast).is_ok());
        prop_assert_eq!(parse_addressee(&rest, &personas), None);
        prop_assert_eq!(resolve_addressee(&rest, &personas, &cast), Some(personas[conductor].id.clone()));
    }

    /// Dowolny ciąg poleceń: każdy wynik zastosowania jest poprawną obsadą, błąd zostawia stan.
    #[test]
    fn commands_preserve_validity(ops in prop::collection::vec((persona_index(), 0usize..9, 0u8..3, any::<bool>()), 0..30), voice in any::<bool>()) {
        let catalog = Catalog::builtin();
        let personas = builtin_personas();
        let roles = catalog.roles().to_vec();
        let mut cast = catalog.default_cast(voice);
        for (p, r, kind, exclusive) in ops {
            let persona = personas[p].id.clone();
            let role = roles[r].id.clone();
            let cmd = match kind {
                0 => CastCommand::Assign { persona, roles: vec![role], exclusive },
                1 => CastCommand::Remove { persona, roles: vec![role] },
                _ => CastCommand::Template { template: catalog.templates()[r % 4].id.clone(), persona: Some(persona) },
            };
            if let Ok(next) = apply_command(&cast, &cmd, &catalog) {
                prop_assert!(catalog.validate_cast(&next).is_ok());
                prop_assert_eq!(next.voice, voice);
                prop_assert_eq!(next.holders(&RoleId::conductor()).len(), 1);
                cast = next;
            }
        }
        // Serde round-trip obsady.
        let json = serde_json::to_string(&cast).unwrap();
        prop_assert_eq!(serde_json::from_str::<Cast>(&json).unwrap(), cast);
    }

    /// Parser nigdy nie panikuje i zwraca tylko znane persony.
    #[test]
    fn parser_is_total(text in "\\PC{0,80}") {
        let catalog = Catalog::builtin();
        let cast = catalog.default_cast(false);
        let known = |p: &PersonaId| catalog.persona(p).is_some();
        match parse_cast_command(&text, &catalog, &cast) {
            Some(CastCommand::Assign { persona, .. } | CastCommand::Remove { persona, .. }) => prop_assert!(known(&persona)),
            Some(CastCommand::Template { template, persona }) => {
                prop_assert!(catalog.template(&template).is_some());
                prop_assert!(persona.as_ref().is_none_or(known));
            }
            None => {}
        }
        if let Some(p) = parse_addressee(&text, catalog.personas()) {
            prop_assert!(known(&p));
        }
    }
}

#[test]
fn export_round_trip() {
    let catalog = Catalog::builtin();
    let mut casts = std::collections::BTreeMap::new();
    casts.insert(
        "s1".to_owned(),
        catalog
            .cast_from_template(&TemplateId::research(), None, true)
            .unwrap(),
    );
    let export = catalog.export(casts);
    let json = serde_json::to_string_pretty(&export).unwrap();
    let back: PersonasExport = serde_json::from_str(&json).unwrap();
    assert_eq!(back, export);
    let schema = schemars::schema_for!(PersonasExport);
    assert!(
        serde_json::to_string(&schema)
            .unwrap()
            .contains("default_template")
    );
}
