//! Katalog wbudowany, walidacja obsad i Kreator (model danych + walidacja).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use personas_contract::{
    Cast, CastError, CastTemplate, CastWarning, Catalog, ColorToken, ModelError, NameForms,
    Persona, PersonaId, Role, RoleId, TemplateId, Verifier, builtin_personas,
};

#[test]
fn builtin_personas_match_personas_md() {
    let personas = builtin_personas();
    let summary: Vec<(String, char, String, u8)> = personas
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                p.glyph,
                p.color.to_string(),
                p.voice.perceived_age,
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("Alfa".into(), 'α', "color.agent.alfa".into(), 23),
            ("Beta".into(), 'β', "color.agent.beta".into(), 22),
            ("Gama".into(), 'γ', "color.agent.gama".into(), 25),
            ("Delta".into(), 'δ', "color.agent.delta".into(), 20),
        ]
    );
    for p in &personas {
        p.validate().unwrap();
        assert_eq!(p.wake_phrases, [format!("Hej {}", p.name)]);
        assert!((18..=25).contains(&p.voice.perceived_age));
    }
    let delta = &personas[3].forms;
    assert_eq!(
        (delta.dative.as_str(), delta.vocative.as_str()),
        ("Delcie", "Delto")
    );
}

#[test]
fn builtin_templates_are_valid_in_text_and_voice() {
    let catalog = Catalog::builtin();
    for template in catalog.templates() {
        for voice in [false, true] {
            let cast = catalog
                .cast_from_template(&template.id, None, voice)
                .unwrap_or_else(|e| panic!("{}: {e}", template.id));
            assert_eq!(cast.voice, voice);
            if voice {
                assert!(cast.speaker().is_some());
            }
        }
    }
    let standard = catalog.default_cast(false);
    assert_eq!(catalog.validate_cast(&standard).unwrap(), Vec::new());
    let solo = catalog
        .cast_from_template(&TemplateId::solo(), Some(&PersonaId::gama()), false)
        .unwrap();
    assert_eq!(solo.roles_of(&PersonaId::gama()).len(), 9);
}

#[test]
fn validation_errors_and_warnings() {
    let catalog = Catalog::builtin();
    let one = |p: &str, roles: &[&str]| -> Cast {
        let set: BTreeSet<RoleId> = roles.iter().map(|r| RoleId::from(*r)).collect();
        Cast::new(None, false, BTreeMap::from([(PersonaId::from(p), set)]))
    };
    assert_eq!(
        catalog.validate_cast(&Cast::default()),
        Err(CastError::Empty)
    );
    assert_eq!(
        catalog.validate_cast(&one("zeta", &["conductor"])),
        Err(CastError::UnknownPersona("zeta".into()))
    );
    assert_eq!(
        catalog.validate_cast(&one("alfa", &["conductor", "szef"])),
        Err(CastError::UnknownRole("szef".into()))
    );
    assert_eq!(
        catalog.validate_cast(&one("alfa", &["coder"])),
        Err(CastError::NoConductor)
    );
    let mut voice = one("alfa", &["conductor"]);
    voice.voice = true;
    assert_eq!(
        catalog.validate_cast(&voice),
        Err(CastError::NoSpeakerInVoiceSession)
    );
    assert_eq!(
        catalog.validate_cast(&one("alfa", &["conductor"])),
        Ok(vec![CastWarning::NoCritic])
    );
    let mut two_speakers = catalog.default_cast(true);
    two_speakers.assign(&PersonaId::beta(), &RoleId::speaker(), false);
    assert!(matches!(
        catalog.validate_cast(&two_speakers),
        Err(CastError::MultipleHolders { .. })
    ));
}

#[test]
fn critic_is_not_the_author_when_possible() {
    let catalog = Catalog::builtin();
    let mut cast = catalog.default_cast(false);
    // Delta (Koderka) przejmuje weryfikację: ostrzeżenie + zastępstwo dla jej własnych wyników.
    cast.assign(&PersonaId::delta(), &RoleId::critic(), true);
    assert_eq!(
        catalog.validate_cast(&cast).unwrap(),
        vec![CastWarning::CriticAlsoAuthor(PersonaId::delta())]
    );
    assert_eq!(
        cast.verifier_for(&PersonaId::beta()),
        Some(Verifier::Critic(PersonaId::delta()))
    );
    assert_eq!(
        cast.verifier_for(&PersonaId::delta()),
        Some(Verifier::Substitute(PersonaId::gama())),
        "zastępczyni: Myślicielka (Gama)"
    );
    let solo = catalog.solo_cast(Some(&PersonaId::beta()), false);
    assert_eq!(
        solo.verifier_for(&PersonaId::beta()),
        Some(Verifier::SelfCheck(PersonaId::beta()))
    );
    let mut no_critic = catalog.default_cast(false);
    no_critic.unassign(&PersonaId::gama(), &RoleId::critic());
    assert_eq!(no_critic.verifier_for(&PersonaId::delta()), None);
}

fn custom_persona() -> Persona {
    let mut p = builtin_personas()[0].clone();
    p.id = PersonaId::new("epsilon");
    p.name = "Epsilon".into();
    p.glyph = 'ε';
    p.color = ColorToken::new("color.agent.epsilon");
    p.wake_phrases = vec!["Hej Epsilon".into()];
    p.forms = NameForms::uniform("Epsilon");
    p.builtin = false;
    p
}

#[test]
fn creator_validates_custom_persona() {
    let mut catalog = Catalog::builtin();
    catalog.add_persona(custom_persona()).unwrap();
    assert!(catalog.persona(&PersonaId::new("epsilon")).is_some());
    assert!(matches!(
        catalog.add_persona(custom_persona()),
        Err(ModelError::Conflict(_))
    ));

    let mut clash = custom_persona();
    clash.id = PersonaId::new("dzeta");
    clash.glyph = 'ζ';
    clash.name = "Dzeta".into();
    clash.wake_phrases = vec!["Hej Dzeta".into()];
    clash.forms = NameForms::uniform("Dzeta");
    clash.forms.dative = "Delcie".into();
    assert!(matches!(
        catalog.add_persona(clash),
        Err(ModelError::Conflict(_))
    ));

    let mut young = custom_persona();
    young.voice.perceived_age = 16;
    assert_eq!(young.validate(), Err(ModelError::AgeOutOfRange(16)));
    let mut words = custom_persona();
    words.voice.design_prompt = "cute girl voice".into();
    assert!(matches!(
        words.validate(),
        Err(ModelError::ForbiddenVoiceWord(_))
    ));
    let mut phrase = custom_persona();
    phrase.wake_phrases = vec!["Hej ty".into()];
    assert!(matches!(
        phrase.validate(),
        Err(ModelError::WakePhraseWithoutName(_))
    ));
    let mut bad_id = custom_persona();
    bad_id.id = PersonaId::new("Epsilon!");
    assert!(matches!(bad_id.validate(), Err(ModelError::InvalidId(_))));
    let mut builtin = custom_persona();
    builtin.builtin = true;
    assert!(matches!(
        catalog.add_persona(builtin),
        Err(ModelError::Builtin(_))
    ));
}

#[test]
fn custom_roles_and_templates() {
    let mut catalog = Catalog::builtin();
    let role = Role {
        id: RoleId::new("ksiegowa"),
        name: "Księgowa".into(),
        description: "faktury".into(),
        prompt: "Jako Księgowa porządkujesz faktury.".into(),
        model_policy: "summarize".into(),
        tools: vec![],
        read_only: false,
        untrusted_isolated: false,
        author: true,
        unique: false,
        builtin: false,
    };
    catalog.add_role(role.clone()).unwrap();
    assert!(matches!(
        catalog.add_role(role),
        Err(ModelError::Conflict(_))
    ));

    let bad = CastTemplate {
        id: TemplateId::new("pusty"),
        name: "Pusty".into(),
        assignments: BTreeMap::new(),
        builtin: false,
    };
    assert!(matches!(
        catalog.add_template(bad),
        Err(ModelError::Conflict(_))
    ));

    let mut restored = Catalog::builtin();
    catalog.add_persona(custom_persona()).unwrap();
    let export = catalog.export(BTreeMap::new());
    assert_eq!((export.personas.len(), export.roles.len()), (1, 1));
    restored.import(&export).unwrap();
    assert_eq!(restored.personas().len(), 5);
    assert_eq!(restored.roles().len(), 10);
}
