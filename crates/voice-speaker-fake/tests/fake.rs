//! Atrapa: kontrakt współdzielony, EER na syntetycznych embeddingach (CI dla F5-07), zgodność
//! modelu profilu, sygnał do klasyfikatora ryzyka.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use risk_classifier_contract::CommandOrigin;
use voice_speaker_contract::eer::report;
use voice_speaker_contract::{
    EnrollmentStatus, Profile, ProfileStore, SpeakerCheck, SpeakerError, SpeakerVerifier,
    contract_tests, voice_origin,
};
use voice_speaker_fake::{
    MemoryProfileStore, PITCH_MODEL, fake_speaker, owner_voice, stranger_voice,
};

#[test]
fn contract_suite() {
    contract_tests::run_all(
        || fake_speaker(MemoryProfileStore::new()).unwrap(),
        &owner_voice,
        &stranger_voice,
    );
}

#[test]
fn eer_on_synthetic_embeddings() {
    let v = fake_speaker(MemoryProfileStore::new()).unwrap();
    v.begin_enrollment().unwrap();
    for seed in 1..=3 {
        v.add_enrollment(&owner_voice(seed)).unwrap();
    }
    v.finish_enrollment().unwrap();
    let genuine: Vec<f32> = (100..130)
        .map(|s| v.verify(&owner_voice(s)).unwrap().score)
        .collect();
    let impostor: Vec<f32> = (200..320)
        .map(|s| v.verify(&stranger_voice(s)).unwrap().score)
        .collect();
    let cfg = v.config();
    let r = report(
        &genuine,
        &impostor,
        cfg.threshold_standard,
        cfg.threshold_strict,
    );
    eprintln!(
        "F5-07 (syntetyczne): EER {:.4} przy progu {:.3}; FAR/FRR przy progach {:?}",
        r.eer, r.eer_threshold, r.at_config
    );
    assert!(r.f5_07_ok && r.f5_08_ok, "{r:?}");
    assert!(
        !r.sufficient_impostors,
        "120 prób obcych < 3000 — FAR 0,1% niewiarygodny"
    );
}

#[test]
fn profile_from_other_model_is_rejected_and_store_persists() {
    let store = MemoryProfileStore::new();
    let v = fake_speaker(store.clone()).unwrap();
    v.begin_enrollment().unwrap();
    for seed in 1..=3 {
        v.add_enrollment(&owner_voice(seed)).unwrap();
    }
    v.finish_enrollment().unwrap();
    assert_eq!(store.saves(), 1);
    // Nowa instancja czyta profil z magazynu.
    let again = fake_speaker(store.clone()).unwrap();
    assert!(matches!(
        again.status(),
        EnrollmentStatus::Enrolled { utterances: 3, ref model } if model == PITCH_MODEL
    ));
    let p = store.load().unwrap().unwrap();
    assert!(
        !format!("{p:?}").contains("0."),
        "Debug profilu bez wartości"
    );
    store
        .save(&Profile {
            model: "ecapa-inny".into(),
            ..p
        })
        .unwrap();
    let other = fake_speaker(store.clone()).unwrap();
    assert!(matches!(
        other.verify(&owner_voice(9)),
        Err(SpeakerError::ModelMismatch { .. })
    ));
    // Niespójna rejestracja (obcy głos w środku) — odrzucona.
    let mixed = fake_speaker(MemoryProfileStore::new()).unwrap();
    mixed.begin_enrollment().unwrap();
    mixed.add_enrollment(&owner_voice(1)).unwrap();
    mixed.add_enrollment(&stranger_voice(3)).unwrap();
    mixed.add_enrollment(&owner_voice(2)).unwrap();
    assert!(matches!(
        mixed.finish_enrollment(),
        Err(SpeakerError::Inconsistent { index: 2 })
    ));
}

#[test]
fn verification_feeds_existing_risk_fields() {
    let v = fake_speaker(MemoryProfileStore::new()).unwrap();
    v.begin_enrollment().unwrap();
    for seed in 1..=4 {
        v.add_enrollment(&owner_voice(seed)).unwrap();
    }
    v.finish_enrollment().unwrap();
    let owner = SpeakerCheck::from_verification(&v.verify(&owner_voice(50)).unwrap());
    let stranger = SpeakerCheck::from_verification(&v.verify(&stranger_voice(50)).unwrap());
    let verified = |c: &SpeakerCheck| match voice_origin(0.9, c) {
        CommandOrigin::UserVoice {
            speaker_verified, ..
        } => speaker_verified,
        _ => unreachable!(),
    };
    assert!(verified(&owner), "{owner:?}");
    assert!(!verified(&stranger), "{stranger:?}");
}
