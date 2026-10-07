//! Współdzielone testy kontraktowe `SpeakerVerifier` (feature `contract-tests`), uruchamiane na
//! `-fake` i `-impl`. `owner(seed)` / `stranger(seed)` dają wypowiedzi 16 kHz (≥ 2 s) dwóch
//! wyraźnie różnych głosów (syntetycznych), rozróżnialnych przez model implementacji.

use risk_classifier_contract::RiskLevel;

use crate::{
    Decision, EnrollmentStatus, ExportConsent, SpeakerError, SpeakerEvent, SpeakerVerifier,
};

/// Generator wypowiedzi (ziarno → audio).
pub type Voice<'a> = &'a dyn Fn(u64) -> Vec<f32>;

/// Bez profilu: weryfikacja i eksport odmawiają; rejestracja wymaga ≥ 3 wypowiedzi.
pub fn enrollment_flow<V: SpeakerVerifier>(v: &V, owner: Voice<'_>) {
    assert_eq!(v.status(), EnrollmentStatus::NotEnrolled);
    assert_eq!(v.verify(&owner(1)).err(), Some(SpeakerError::NotEnrolled));
    assert_eq!(
        v.add_enrollment(&owner(1)).err(),
        Some(SpeakerError::NotEnrolling)
    );
    v.begin_enrollment().unwrap_or_else(|e| panic!("{e}"));
    let short: Vec<f32> = owner(1).into_iter().take(4_000).collect();
    assert!(matches!(
        v.add_enrollment(&short),
        Err(SpeakerError::TooShort { .. })
    ));
    assert_eq!(
        v.add_enrollment(&vec![0.0; 48_000]).err(),
        Some(SpeakerError::TooQuiet)
    );
    for seed in 1..=2 {
        let p = v
            .add_enrollment(&owner(seed))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(!p.ready);
    }
    assert!(matches!(
        v.finish_enrollment(),
        Err(SpeakerError::NotEnoughUtterances { have: 2, need: 3 })
    ));
    let p = v
        .add_enrollment(&owner(3))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(p.ready && p.done == 3);
    let st = v.finish_enrollment().unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(
        st,
        EnrollmentStatus::Enrolled { utterances: 3, .. }
    ));
}

/// Właściciel przechodzi, obcy nie; progi zależne od ryzyka.
pub fn verification_separates<V: SpeakerVerifier>(v: &V, owner: Voice<'_>, stranger: Voice<'_>) {
    let cfg = v.config();
    for seed in 10..14 {
        let ok = v.verify(&owner(seed)).unwrap_or_else(|e| panic!("{e}"));
        assert_ne!(ok.decision, Decision::Rejected, "właściciel {seed}: {ok:?}");
        assert!(ok.accepts(RiskLevel::Low, &cfg));
        assert!(ok.confidence > 0.5);
        let bad = v.verify(&stranger(seed)).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(bad.decision, Decision::Rejected, "obcy {seed}: {bad:?}");
        assert!(!bad.accepts(RiskLevel::High, &cfg) && bad.confidence < 0.5);
        assert!(bad.score < ok.score);
    }
    let short: Vec<f32> = owner(20).into_iter().take(1_600).collect();
    assert!(matches!(
        v.verify(&short),
        Err(SpeakerError::TooShort { .. })
    ));
}

/// Eksport tylko za jawną zgodą; usuwanie kasuje profil; zdarzenia bez audio.
pub fn export_and_delete<V: SpeakerVerifier>(v: &V, owner: Voice<'_>) {
    assert_eq!(v.export(None).err(), Some(SpeakerError::ConsentRequired));
    let fake_consent = ExportConsent {
        confirmed_by_user: false,
        at_unix_ms: 1,
        purpose: "x".into(),
    };
    assert_eq!(
        v.export(Some(&fake_consent)).err(),
        Some(SpeakerError::ConsentRequired)
    );
    let consent = ExportConsent::explicit(1_760_000_000_000, "przeniesienie na laptop");
    let e = v.export(Some(&consent)).unwrap_or_else(|e| panic!("{e}"));
    assert!(!e.embedding.is_empty() && e.utterances >= 3);
    assert!(!format!("{e:?}").contains("0."), "Debug bez wartości");
    assert!(v.delete().unwrap_or_else(|e| panic!("{e}")));
    assert_eq!(v.status(), EnrollmentStatus::NotEnrolled);
    assert_eq!(v.verify(&owner(30)).err(), Some(SpeakerError::NotEnrolled));
    assert!(!v.delete().unwrap_or_else(|e| panic!("{e}")));
    let events = v.take_events();
    assert!(events.contains(&SpeakerEvent::Deleted));
    assert!(events.contains(&SpeakerEvent::Export { granted: true }));
    assert!(events.contains(&SpeakerEvent::Export { granted: false }));
    for ev in &events {
        let payload = ev.to_bus_event().payload.to_string();
        assert!(
            payload.len() < 300,
            "zdarzenie bez audio/embeddingu: {payload}"
        );
    }
    v.begin_enrollment().unwrap_or_else(|e| panic!("{e}"));
    v.add_enrollment(&owner(40))
        .unwrap_or_else(|e| panic!("{e}"));
    v.cancel_enrollment();
    assert_eq!(v.status(), EnrollmentStatus::NotEnrolled);
}

/// Cały zestaw na świeżej instancji z `factory`.
pub fn run_all<V: SpeakerVerifier, F: Fn() -> V>(
    factory: F,
    owner: Voice<'_>,
    stranger: Voice<'_>,
) {
    let v = factory();
    enrollment_flow(&v, owner);
    verification_separates(&v, owner, stranger);
    export_and_delete(&v, owner);
}
