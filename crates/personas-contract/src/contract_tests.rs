//! Współdzielony test kontraktowy (feature `contract-tests`): ten sam zestaw dla `-impl` i `-fake`.

use core_bus_contract::SessionId;

use crate::{
    CastError, ChangeOrigin, PersonaId, Personas, PersonasError, RoleId, TemplateId, Verifier,
};

fn session() -> SessionId {
    SessionId::new("s-kontrakt")
}

/// Katalog wbudowany: 4 persony z glifami i tokenami kolorów, 9 ról, 4 szablony.
pub async fn builtin_catalog<P: Personas>(p: &P) {
    let personas = p.personas();
    let glyphs: String = personas.iter().map(|x| x.glyph).collect();
    assert_eq!(glyphs, "αβγδ");
    assert!(
        personas
            .iter()
            .all(|x| x.color.as_str().starts_with("color.agent."))
    );
    assert_eq!(p.roles().len(), 9);
    let templates: Vec<String> = p.templates().iter().map(|t| t.id.to_string()).collect();
    assert_eq!(templates, ["standard", "solo", "coding", "research"]);
}

/// Nowa sesja ma obsadę „Standard”: Alfa = Dyrygentka + Mówczyni.
pub async fn default_cast_is_standard<P: Personas>(p: &P) {
    let cast = p.cast(&session());
    assert_eq!(cast.template, Some(TemplateId::standard()));
    assert_eq!(cast.conductor(), Some(PersonaId::alfa()));
    assert_eq!(cast.speaker(), Some(PersonaId::alfa()));
    assert_eq!(
        cast.verifier_for(&PersonaId::delta()),
        Some(Verifier::Critic(PersonaId::gama()))
    );
}

/// Zmiana obsady w locie głosem: „Beta, teraz ty prowadzisz” → Beta Dyrygentką.
pub async fn voice_command_changes_cast<P: Personas>(p: &P) {
    let s = session();
    let change = p
        .apply_command(&s, "Beta, teraz ty prowadzisz", ChangeOrigin::Voice)
        .await
        .unwrap_or_else(|e| panic!("{e}"))
        .unwrap_or_else(|| panic!("to jest polecenie obsady"));
    assert_eq!(change.after.conductor(), Some(PersonaId::beta()));
    assert!(
        change
            .diff
            .assigned
            .contains(&(PersonaId::beta(), RoleId::conductor()))
    );
    assert!(
        change
            .diff
            .removed
            .contains(&(PersonaId::alfa(), RoleId::conductor()))
    );
    assert_eq!(p.cast(&s).conductor(), Some(PersonaId::beta()));
    // Bez imienia odpowiada nowa Dyrygentka; imię wygrywa.
    assert_eq!(p.resolve_addressee(&s, "co dalej?"), PersonaId::beta());
    assert_eq!(
        p.resolve_addressee(&s, "Delto, co dalej?"),
        PersonaId::delta()
    );
    let none = p
        .apply_command(&s, "Beta, jak się masz?", ChangeOrigin::Voice)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(none, None);
}

/// Niepoprawna obsada jest odrzucana, a poprzednia zostaje.
pub async fn invalid_cast_rejected<P: Personas>(p: &P) {
    let s = session();
    let before = p.cast(&s);
    let mut two = before.clone();
    two.assign(&PersonaId::gama(), &RoleId::conductor(), false);
    let err = p.set_cast(&s, two, ChangeOrigin::Ui).await;
    assert!(matches!(
        err,
        Err(PersonasError::Cast(CastError::MultipleHolders { .. }))
    ));
    let mut no_speaker = before.clone();
    no_speaker.voice = true;
    no_speaker.unassign(&PersonaId::alfa(), &RoleId::speaker());
    let err = p.set_cast(&s, no_speaker, ChangeOrigin::Ui).await;
    assert_eq!(
        err.map(|_| ()),
        Err(PersonasError::Cast(CastError::NoSpeakerInVoiceSession))
    );
    assert_eq!(p.cast(&s), before);
}

/// Tryb głosowy: sesja z szablonu bez Mówczyni dostaje ją u Dyrygentki.
pub async fn voice_mode_ensures_speaker<P: Personas>(p: &P) {
    let s = SessionId::new("s-glos");
    let mut cast = p.cast(&s);
    cast.unassign(&PersonaId::alfa(), &RoleId::speaker());
    p.set_cast(&s, cast, ChangeOrigin::Ui)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let change = p
        .set_voice(&s, true, ChangeOrigin::System)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(change.after.voice);
    assert_eq!(change.after.speaker(), Some(PersonaId::alfa()));
}

/// Prompt systemowy w rodzaju żeńskim z rolami z obsady.
pub async fn system_prompt_has_roles<P: Personas>(p: &P) {
    let prompt = p
        .system_prompt(&session(), &PersonaId::gama())
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(prompt.contains("Gama") && prompt.contains("Krytyczka"));
    assert!(prompt.contains("rodzaju żeńskim"));
    assert!(matches!(
        p.system_prompt(&session(), &PersonaId::new("nikt")),
        Err(PersonasError::UnknownPersona(_))
    ));
}

/// Szablon „Solo” poleceniem i eksport obsady do `.alfa`.
pub async fn solo_and_export<P: Personas>(p: &P) {
    let s = SessionId::new("s-solo");
    let change = p
        .apply_command(&s, "Delta, zrób wszystko sama", ChangeOrigin::Text)
        .await
        .unwrap_or_else(|e| panic!("{e}"))
        .unwrap_or_else(|| panic!("polecenie Solo"));
    assert_eq!(change.after.personas(), vec![PersonaId::delta()]);
    assert_eq!(
        change.after.verifier_for(&PersonaId::delta()),
        Some(Verifier::SelfCheck(PersonaId::delta()))
    );
    let export = p.export();
    assert_eq!(export.casts.get("s-solo"), Some(&change.after));
    assert!(
        export.personas.is_empty(),
        "eksport zawiera tylko własne persony"
    );
}

/// Uruchamia cały zestaw; `factory` daje świeżą, uruchomioną instancję.
pub async fn run_all<P, F, Fut>(factory: F)
where
    P: Personas,
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = P>,
{
    builtin_catalog(&factory().await).await;
    default_cast_is_standard(&factory().await).await;
    voice_command_changes_cast(&factory().await).await;
    invalid_cast_rejected(&factory().await).await;
    voice_mode_ensures_speaker(&factory().await).await;
    system_prompt_has_roles(&factory().await).await;
    solo_and_export(&factory().await).await;
}
