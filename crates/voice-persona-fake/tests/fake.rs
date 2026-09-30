//! Testy atrapy: kontrakt współdzielony + zachowania specyficzne dla atrapy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use voice_persona_contract::{
    EngineKind, EngineStyleTable, Lexicon, Persona, PersonaId, contract_tests,
};
use voice_persona_fake::{FakePersona, table_normalize};

#[test]
fn contract_suite() {
    contract_tests::run_all(FakePersona::new);
}

#[test]
fn table_normalizer_reads_digits_one_by_one() {
    assert_eq!(
        table_normalize("Mam 12 kotów.", &Lexicon::new()),
        "Mam jeden dwa kotów."
    );
    assert_eq!(table_normalize("x5", &Lexicon::new()), "x pięć");
}

#[test]
fn records_plans_with_neutral_style() {
    let fake = FakePersona::new();
    let table = EngineStyleTable::neutral(EngineKind::PocketTts);
    let plan = fake
        .plan(&PersonaId::beta(), "Linia 1\n\nLinia 2", &table)
        .unwrap();
    assert_eq!(plan.sentences.len(), 2);
    assert_eq!(plan.sentences[0].text, "Linia jeden");
    assert!(plan.sentences.iter().all(|s| s.style.rate == 1.0));
    assert_eq!(fake.planned().len(), 1);
}
