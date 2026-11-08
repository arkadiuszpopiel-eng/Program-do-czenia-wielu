//! Parser poleceń obsady — testy tabelaryczne (≥ 30 przypadków PL) i skutki na obsadzie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use personas_contract::{
    Cast, CastCommand, CastError, Catalog, PersonaId, RoleId, TemplateId, apply_command,
    parse_cast_command,
};

/// Oczekiwany wynik parsowania w skrócie: `A:persona:rola,rola[:+]`, `R:persona:rola`, `T:szablon[:persona]`.
fn short(cmd: Option<CastCommand>) -> String {
    match cmd {
        None => "-".into(),
        Some(CastCommand::Assign {
            persona,
            roles,
            exclusive,
        }) => {
            let roles: Vec<&str> = roles.iter().map(RoleId::as_str).collect();
            format!(
                "A:{persona}:{}{}",
                roles.join(","),
                if exclusive { "" } else { ":+" }
            )
        }
        Some(CastCommand::Remove { persona, roles }) => {
            let roles: Vec<&str> = roles.iter().map(RoleId::as_str).collect();
            format!("R:{persona}:{}", roles.join(","))
        }
        Some(CastCommand::Template { template, persona }) => match persona {
            Some(p) => format!("T:{template}:{p}"),
            None => format!("T:{template}"),
        },
    }
}

const CASES: &[(&str, &str)] = &[
    // Dyrygentka
    ("Beta, teraz ty prowadzisz", "A:beta:conductor"),
    ("Beta, teraz ty prowadzisz.", "A:beta:conductor"),
    ("beta teraz ty prowadzisz", "A:beta:conductor"),
    ("Niech Gama prowadzi", "A:gama:conductor"),
    ("Od teraz Delta jest dyrygentką", "A:delta:conductor"),
    ("Gamo, przejmij prowadzenie", "A:gama:conductor"),
    ("Przekaż prowadzenie Delcie", "A:delta:conductor"),
    ("Alfa, oddaj prowadzenie Becie", "A:beta:conductor"),
    ("Wyznaczam Betę na dyrygentkę", "A:beta:conductor"),
    ("Delta, zostań koordynatorką", "A:delta:conductor"),
    ("Beto, możesz teraz ty prowadzić?", "A:beta:conductor"),
    // Krytyczka
    ("Delta, przejmij weryfikację", "A:delta:critic"),
    ("Delta, przejmij weryfikacje", "A:delta:critic"),
    ("Niech Beta sprawdza wyniki", "A:beta:critic"),
    ("Alfo, zostań krytyczką", "A:alfa:critic"),
    ("Daj Gamie recenzję", "A:gama:critic"),
    ("Delta, przestań weryfikować", "R:delta:critic"),
    ("Gama, już nie jesteś krytyczką", "R:gama:critic"),
    // Pozostałe role
    ("Alfa, przejmij rozmowę", "A:alfa:speaker"),
    ("Przekaż głos Delcie", "A:delta:speaker"),
    ("Delta, zajmij się kodem", "A:delta:coder"),
    ("Beta, ty też kodujesz", "A:beta:coder:+"),
    ("Gama, przejmij planowanie", "A:gama:thinker"),
    ("Beta, przejmij badania", "A:beta:researcher"),
    ("Delta, teraz ty notujesz", "A:delta:keeper"),
    ("Gama, przejmij tłumaczenie", "A:gama:writer"),
    ("Beta, obejmij obsługę komputera", "A:beta:operator"),
    (
        "Delta, przejmij prowadzenie i rozmowę",
        "A:delta:conductor,speaker",
    ),
    // Szablony
    ("Obsada solo", "T:solo:alfa"),
    ("Alfa, zrób wszystko sama", "T:solo:alfa"),
    ("Beta, pracuj solo", "T:solo:beta"),
    ("Przełącz na obsadę kodowanie", "T:coding"),
    ("Włącz tryb badania", "T:research"),
    ("Wróć do obsady standardowej", "T:standard"),
    // Nie-polecenia
    ("Beta, jak się masz?", "-"),
    ("Kto teraz prowadzi?", "-"),
    ("Delta, sprawdź pogodę", "-"),
    ("Gama prowadzi badania nad pamięcią", "-"),
    ("Beta, ty mówisz za szybko", "-"),
    ("Beta, teraz sprawdź kod", "-"),
    ("Przekazuję Delcie", "-"),
    ("teraz ty prowadzisz", "-"),
    ("Jaka jest pogoda w Krakowie", "-"),
    ("", "-"),
];

#[test]
fn table_of_polish_commands() {
    assert!(CASES.len() >= 30, "za mało przypadków: {}", CASES.len());
    let catalog = Catalog::builtin();
    let cast = catalog.default_cast(false);
    let mut failures = Vec::new();
    for (text, expected) in CASES {
        let got = short(parse_cast_command(text, &catalog, &cast));
        if got != *expected {
            failures.push(format!("{text:?}: oczekiwano {expected}, jest {got}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn apply(text: &str, cast: &Cast) -> Result<Cast, CastError> {
    let catalog = Catalog::builtin();
    let cmd = parse_cast_command(text, &catalog, cast).expect("polecenie");
    apply_command(cast, &cmd, &catalog)
}

#[test]
fn effects_on_standard_cast() {
    let catalog = Catalog::builtin();
    let standard = catalog.default_cast(false);

    let beta_leads = apply("Beta, teraz ty prowadzisz", &standard).unwrap();
    assert_eq!(beta_leads.conductor(), Some(PersonaId::beta()));
    assert_eq!(
        beta_leads.speaker(),
        Some(PersonaId::alfa()),
        "Mówczyni bez zmian"
    );
    assert_eq!(beta_leads.template, None);

    let delta_critic = apply("Delta, przejmij weryfikację", &standard).unwrap();
    assert_eq!(
        delta_critic.holders(&RoleId::critic()),
        vec![PersonaId::delta()]
    );
    assert!(
        !delta_critic
            .roles_of(&PersonaId::gama())
            .contains(&RoleId::critic())
    );
    let warnings = catalog.validate_cast(&delta_critic).unwrap();
    assert!(
        !warnings.is_empty(),
        "Delta jest Koderką i Krytyczką — ostrzeżenie"
    );

    let also = apply("Beta, ty też kodujesz", &standard).unwrap();
    assert_eq!(
        also.holders(&RoleId::coder()),
        vec![PersonaId::beta(), PersonaId::delta()]
    );

    // Odebranie jedynej Dyrygentki jest odrzucane walidacją.
    let err = apply("Alfa, przestań prowadzić", &standard);
    assert_eq!(err, Err(CastError::NoConductor));

    let coding = apply("Przełącz na obsadę kodowanie", &standard).unwrap();
    assert_eq!(coding.template, Some(TemplateId::coding()));
    assert_eq!(coding.conductor(), Some(PersonaId::delta()));
}

#[test]
fn custom_template_by_name() {
    let mut catalog = Catalog::builtin();
    let mut tpl = catalog.template(&TemplateId::standard()).unwrap().clone();
    tpl.id = TemplateId::new("wieczor");
    tpl.name = "Wieczór".into();
    tpl.builtin = false;
    catalog.add_template(tpl).unwrap();
    let cast = catalog.default_cast(false);
    let cmd = parse_cast_command("Włącz obsadę wieczór", &catalog, &cast);
    assert_eq!(short(cmd), "T:wieczor");
}
